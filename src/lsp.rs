//! A Language Server for the bgraph DSL, spoken over stdio.
//!
//! JSON-RPC framing is hand-rolled: the protocol surface we need is small and
//! a synchronous request loop is the honest shape for a server whose entire
//! workload is "re-parse a file that fits in a screen".
//!
//! Everything semantic comes from `parser` + `check`, so the squiggles in the
//! editor are exactly the errors the CLI would print. When a document is
//! mid-edit and does not parse, completion and navigation fall back to a
//! lexical scan — a broken file is precisely when help matters most.

use std::collections::HashMap;
use std::io::BufReader;

use serde_json::{Value, json};

#[cfg(test)]
use crate::props;

mod positions;
pub use positions::LineIndex;
mod completion;
mod completion_engine;
mod context;
#[path = "lsp/diagnostics.rs"]
mod diagnostic_engine;
mod hover;
mod navigation;
mod protocol;
use completion::{item as completion, item_snippet, snippet};
use context::{
    Block, Context, declaration_span, declared, declared_archetype, declared_items, is_ident_char,
    is_rel_dir, item_words, text_of, word_at,
};
#[allow(unused_imports)]
pub(crate) use diagnostic_engine::{diagnostics, diagnostics_for, path_of};

// ------------------------------------------------------------ server

#[derive(Default)]
struct Server {
    docs: HashMap<String, String>,
}

pub fn run() -> std::io::Result<()> {
    let stdin = std::io::stdin();
    let mut reader = BufReader::new(stdin.lock());
    let stdout = std::io::stdout();
    let mut writer = stdout.lock();
    let mut server = Server::default();

    while let Some(message) = protocol::read_message(&mut reader)? {
        let id = message.get("id").cloned();
        let method = message
            .get("method")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let params = message.get("params").cloned().unwrap_or(Value::Null);

        if method == "exit" {
            break;
        }

        match server.handle(&method, &params) {
            Response::Result(result) => {
                if let Some(id) = id {
                    protocol::write_message(
                        &mut writer,
                        &json!({ "jsonrpc": "2.0", "id": id, "result": result }),
                    )?;
                }
            }
            Response::Nothing => {
                if let Some(id) = id {
                    protocol::write_message(
                        &mut writer,
                        &json!({ "jsonrpc": "2.0", "id": id, "result": Value::Null }),
                    )?;
                }
            }
            Response::Error { code, message } => {
                if let Some(id) = id {
                    protocol::write_message(
                        &mut writer,
                        &json!({
                            "jsonrpc": "2.0",
                            "id": id,
                            "error": { "code": code, "message": message }
                        }),
                    )?;
                }
            }
        }

        // Notifications the client must receive after the request is answered.
        for note in server.drain_notifications(&method, &params) {
            protocol::write_message(&mut writer, &note)?;
        }
    }

    Ok(())
}

enum Response {
    Result(Value),
    Nothing,
    Error { code: i64, message: &'static str },
}

impl Server {
    fn handle(&mut self, method: &str, params: &Value) -> Response {
        match method {
            "initialize" => Response::Result(protocol::capabilities()),
            "shutdown" => Response::Result(Value::Null),

            "textDocument/didOpen" => {
                let uri = uri_of(params);
                let text = params["textDocument"]["text"].as_str().unwrap_or("");
                self.docs.insert(uri, text.to_string());
                Response::Nothing
            }
            "textDocument/didChange" => {
                let uri = uri_of(params);
                // The server advertises full sync, so the last change is the
                // whole document.
                if let Some(text) = params["contentChanges"]
                    .as_array()
                    .and_then(|c| c.last())
                    .and_then(|c| c["text"].as_str())
                {
                    self.docs.insert(uri, text.to_string());
                }
                Response::Nothing
            }
            "textDocument/didClose" => {
                self.docs.remove(&uri_of(params));
                Response::Nothing
            }
            "textDocument/didSave" => Response::Nothing,

            "textDocument/completion" => Response::Result(self.completion(params)),
            "textDocument/formatting" => {
                let Some(text) = self.docs.get(&uri_of(params)) else {
                    return Response::Result(Value::Null);
                };
                let Ok(formatted) = crate::fmt::format(text) else {
                    return Response::Result(Value::Null);
                };
                let index = LineIndex::new(text);
                let range = index.range(crate::ast::Span {
                    start: 0,
                    end: text.len(),
                });
                Response::Result(if formatted == *text {
                    json!([])
                } else {
                    json!([{"range": range, "newText": formatted}])
                })
            }
            "textDocument/hover" => Response::Result(self.hover(params)),
            "textDocument/definition" => Response::Result(self.definition(params)),
            "textDocument/references" => Response::Result(self.references(params)),
            "textDocument/documentSymbol" => Response::Result(self.symbols(params)),
            "textDocument/signatureHelp" => Response::Result(self.signature(params)),
            "textDocument/documentColor" => Response::Result(self.colours(params)),
            "textDocument/colorPresentation" => Response::Result(colour_presentation(params)),
            "textDocument/documentHighlight" => Response::Result(self.highlights(params)),

            _ => Response::Error {
                code: -32601,
                message: "Method not found",
            },
        }
    }

    fn drain_notifications(&self, method: &str, params: &Value) -> Vec<Value> {
        if !matches!(
            method,
            "textDocument/didOpen"
                | "textDocument/didChange"
                | "textDocument/didSave"
                | "textDocument/didClose"
        ) {
            return Vec::new();
        }

        let uri = uri_of(params);
        if method == "textDocument/didClose" {
            return vec![json!({
                "jsonrpc": "2.0",
                "method": "textDocument/publishDiagnostics",
                "params": { "uri": uri, "diagnostics": [] }
            })];
        }
        let Some(text) = self.docs.get(&uri) else {
            return Vec::new();
        };

        vec![json!({
            "jsonrpc": "2.0",
            "method": "textDocument/publishDiagnostics",
            "params": {
                "uri": uri,
                "diagnostics": diagnostics_for(text, path_of(&uri).as_deref()),
            }
        })]
    }

    fn doc(&self, params: &Value) -> Option<(&String, LineIndex, usize)> {
        let uri = params["textDocument"]["uri"].as_str()?;
        let text = self.docs.get(uri)?;
        let index = LineIndex::new(text);
        let line = params["position"]["line"].as_u64()? as usize;
        let col = params["position"]["character"].as_u64()? as usize;
        let offset = index.offset(line, col);
        Some((text, index, offset))
    }

    // ----------------------------------------------------- signature help

    /// What the statement under the cursor takes, and which value you are on.
    ///
    /// `slice a "A" 4820` is the idiomatic form (`D-015`), and until now the
    /// only way to learn its shape was to read the manual — the editor knew
    /// and did not say. The active parameter is counted from the values
    /// already written, so it walks along as you type.
    fn signature(&self, params: &Value) -> Value {
        let Some((text, _index, offset)) = self.doc(params) else {
            return Value::Null;
        };
        completion::signature(text, offset)
    }

    // ---------------------------------------------------------- colours

    /// Every colour literal in the file, so the editor can show a swatch and
    /// open a picker on it. `#` is only ever a colour here (`D-004`), which is
    /// what makes a lexical scan safe.
    fn colours(&self, params: &Value) -> Value {
        let Some(text) = params["textDocument"]["uri"]
            .as_str()
            .and_then(|uri| self.docs.get(uri))
        else {
            return json!([]);
        };
        completion::colours(text)
    }

    // -------------------------------------------------------- highlights

    /// Every place the identifier under the cursor is written. Reading a
    /// diagram means following an id around the file, and until now that was
    /// the editor's find-in-file rather than anything that knew what an id is.
    fn highlights(&self, params: &Value) -> Value {
        let Some((text, index, offset)) = self.doc(params) else {
            return json!([]);
        };
        navigation::highlights(text, &index, offset)
    }

    // -------------------------------------------------------- hover

    fn hover(&self, params: &Value) -> Value {
        let Some((text, index, offset)) = self.doc(params) else {
            return Value::Null;
        };
        hover::file_hover(text, &index, offset, &uri_of(params))
    }

    // -------------------------------------------------------- navigation

    fn definition(&self, params: &Value) -> Value {
        let Some((text, index, offset)) = self.doc(params) else {
            return Value::Null;
        };
        let uri = params["textDocument"]["uri"].as_str().unwrap_or("");
        navigation::definition(text, &index, offset, uri)
    }

    fn references(&self, params: &Value) -> Value {
        let Some((text, _, offset)) = self.doc(params) else {
            return json!([]);
        };
        navigation::references(
            text,
            offset,
            &uri_of(params),
            params["context"]["includeDeclaration"]
                .as_bool()
                .unwrap_or(false),
        )
    }

    fn symbols(&self, params: &Value) -> Value {
        let Some(uri) = params["textDocument"]["uri"].as_str() else {
            return json!([]);
        };
        let Some(text) = self.docs.get(uri) else {
            return json!([]);
        };
        navigation::symbols(text)
    }
}

// ------------------------------------------------------------ lexical helpers

// ------------------------------------------------------------ protocol

fn uri_of(params: &Value) -> String {
    params["textDocument"]["uri"]
        .as_str()
        .unwrap_or_default()
        .to_string()
}

/// The colour a picker produced, written back as the shortest hex that says
/// it — `#38bdf8` rather than `#38bdf8ff`, since alpha defaults to opaque
/// (`D-016`).
fn colour_presentation(params: &Value) -> Value {
    let c = &params["color"];
    let byte = |key: &str| (c[key].as_f64().unwrap_or(0.0) * 255.0).round() as u8;
    let (r, g, b, a) = (byte("red"), byte("green"), byte("blue"), byte("alpha"));
    let label = if a == 255 {
        format!("#{r:02x}{g:02x}{b:02x}")
    } else {
        format!("#{r:02x}{g:02x}{b:02x}{a:02x}")
    };
    json!([{ "label": label }])
}

#[cfg(test)]
#[path = "lsp/tests.rs"]
mod tests;

#[cfg(test)]
mod block_vocabulary_tests {
    use super::*;

    fn words_after_kind(source: &str) -> Vec<String> {
        let server = Server {
            docs: HashMap::new(),
        };
        server
            .items_for(source, "  kind: ", source.len())
            .iter()
            .filter_map(|i| i["label"].as_str().map(str::to_string))
            .collect()
    }

    #[test]
    fn kind_completes_the_declared_archetypes_words_and_not_the_others() {
        // The whole reason an archetype may refine a shared property: one word
        // means different things in two genres, and offering both would be
        // exactly the noise that made autocompletion get in the way.
        let machine = words_after_kind("type state\nstate a \"A\" {\n  kind: ");
        assert!(machine.contains(&"initial".to_string()), "{machine:?}");
        assert!(!machine.contains(&"person".to_string()), "{machine:?}");

        let architecture = words_after_kind("type block\nblock a \"A\" {\n  kind: ");
        assert!(
            architecture.contains(&"person".to_string()),
            "{architecture:?}"
        );
        assert!(
            !architecture.contains(&"initial".to_string()),
            "{architecture:?}"
        );
    }

    #[test]
    fn a_block_only_property_stays_out_of_other_genres() {
        let server = Server {
            docs: HashMap::new(),
        };
        let names = |source: &str, line: &str| -> Vec<String> {
            server
                .items_for(source, line, source.len())
                .iter()
                .filter_map(|i| i["label"].as_str().map(str::to_string))
                .collect()
        };

        let architecture = names("type block\nblock a \"A\" {\n  ", "  ");
        assert!(
            architecture.contains(&"tech".to_string()),
            "{architecture:?}"
        );

        let free = names("node a \"A\" {\n  ", "  ");
        assert!(!free.contains(&"tech".to_string()), "{free:?}");
        // The shared vocabulary is still all there.
        assert!(free.contains(&"fill".to_string()), "{free:?}");
    }
}

#[cfg(test)]
mod archetype_completion_tests {
    use super::*;

    fn offered(source: &str, line: &str) -> Vec<String> {
        let server = Server {
            docs: HashMap::new(),
        };
        server
            .items_for(source, line, source.len())
            .iter()
            .filter_map(|i| i["label"].as_str().map(str::to_string))
            .collect()
    }

    #[test]
    fn each_genre_offers_its_own_data_and_no_one_elses() {
        // The filter this replaces knew two properties out of thirteen, and
        // had one of them wrong: it hid `value` in a treemap, where it is
        // required.
        for (source, item, wanted, unwanted) in [
            ("type treemap\nbox a \"A\" {\n  ", "box", "value", "wave"),
            ("type pie\nslice a \"A\" {\n  ", "slice", "value", "bits"),
            ("type wave\nsignal s \"S\" {\n  ", "signal", "wave", "value"),
            ("type quadrant\npoint p \"P\" {\n  ", "point", "x", "points"),
            ("type gantt\ntask t \"T\" {\n  ", "task", "start", "value"),
            ("type packet\nfield f \"F\" {\n  ", "field", "bits", "start"),
            ("type bar\nseries s {\n  ", "series", "points", "wave"),
            (
                "type timeline\nevent e \"E\" {\n  ",
                "event",
                "date",
                "start",
            ),
            ("type block\nblock b \"B\" {\n  ", "block", "tech", "wave"),
        ] {
            let names = offered(source, "  ");
            assert!(
                names.iter().any(|n| n == wanted),
                "`{item}` should offer `{wanted}`: {names:?}"
            );
            assert!(
                !names.iter().any(|n| n == unwanted),
                "`{item}` should not offer `{unwanted}`"
            );
            // The shared vocabulary is available everywhere, as it should be.
            assert!(names.iter().any(|n| n == "fill"), "`{item}` lost `fill`");
        }
    }

    #[test]
    fn every_archetype_completes_after_type_and_offers_its_own_keyword() {
        let names = offered("type ", "type ");
        for def in props::ARCHETYPES {
            assert!(
                names.iter().any(|n| n == def.name),
                "`type` completion is missing `{}`: {names:?}",
                def.name
            );
        }

        // And inside a file, the archetype's own word leads a statement.
        for def in props::ARCHETYPES {
            let source = format!("type {}\n", def.name);
            let offered = offered(&source, "");
            assert!(
                offered.iter().any(|n| n == def.item),
                "`type {}` should offer `{}`: {offered:?}",
                def.name,
                def.item
            );
        }
    }

    #[test]
    fn an_item_block_is_not_mistaken_for_an_edge_block() {
        // Every archetype names its elements its own way, and the block owner
        // used to be matched against a list of five words — so `slice`, `box`,
        // `signal`, `task`, `field`, `series`, `event`, `entity`, `point` and
        // the rest all fell through to "edge", losing `fill`, `shape` and
        // `size` and gaining `arrow` and `router`.
        for def in props::ARCHETYPES {
            let source = format!("type {}\n{} a \"A\" {{\n  ", def.name, def.item);
            let names = offered(&source, "  ");
            assert!(
                names.iter().any(|n| n == "shape"),
                "`{} {{ … }}` should offer node properties: {names:?}",
                def.item
            );
            assert!(
                !names.iter().any(|n| n == "router"),
                "`{} {{ … }}` should not offer edge properties",
                def.item
            );
        }

        // And an edge still reads as one, label or no label.
        for head in [
            "a -> b {\n  ",
            "a -> b \"calls\" {\n  ",
            "a <-> b \"a -> b\" {\n  ",
            "a ..> b {\n  ",
        ] {
            let names = offered(head, "  ");
            assert!(
                names.iter().any(|n| n == "router"),
                "`{head}` should offer edge properties: {names:?}"
            );
        }

        // A label holding an arrow does not make an item into an edge.
        let names = offered("node a \"from -> to\" {\n  ", "  ");
        assert!(names.iter().any(|n| n == "shape"), "{names:?}");
        assert!(!names.iter().any(|n| n == "router"), "{names:?}");
    }

    #[test]
    fn hover_documents_a_property_in_the_genre_that_reads_it() {
        let server = Server {
            docs: HashMap::new(),
        };
        let text = "type timeline\nevent e \"E\" {\n  date: 2026-03-01\n}\n";
        let mut server = server;
        server.docs.insert("file:///t.bd".into(), text.into());
        let hover = server.hover(&json!({
            "textDocument": { "uri": "file:///t.bd" },
            "position": { "line": 2, "character": 3 },
        }));
        let value = hover["contents"]["value"].as_str().unwrap_or_default();
        assert!(value.contains("date: <date>"), "{value}");
        assert!(value.contains("When the event happened"), "{value}");
    }
}

#[cfg(test)]
#[path = "lsp/inspection_tests.rs"]
mod inspection_tests;
