//! W-164 contracts exercised through rendering, explain and the LSP binary.
#![cfg(all(feature = "cli", feature = "bpmn", feature = "xy"))]

use bgraph::style::{Registry, Shape, Style};
use serde_json::{Value, json};
use std::io::{BufRead, BufReader, Read, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

const FORMS: &[(&str, &str, &str)] = &[
    ("bpmn", "task", "plain"),
    ("bpmn", "event", "intermediate plain"),
    ("bpmn", "gateway", "exclusive"),
    ("bpmn", "data", "object"),
    ("bpmn", "node", "plain"),
    ("xy", "series", "0 10 1 20"),
    ("xy", "scatter", "0 10 1 20"),
    ("xy", "node", "0 10 1 20"),
];

fn target(genre: &str, word: &str) -> String {
    bgraph::props::archetype(genre)
        .unwrap()
        .style_word(word)
        .to_string()
}

fn styled_source(
    genre: &str,
    word: &str,
    data: &str,
    level: usize,
) -> (String, &'static str, String) {
    let target = target(genre, word);
    let equal = level == 4;
    let color = |normal| if equal { "#404040" } else { normal };
    let mut source = format!(
        "type {genre}\nstyle node {{ fill: {} font-color: #202020 stroke: #303030 }}\nstyle {target} {{ fill: #404040 }}\n",
        color("#101010"),
    );
    let mut classes = String::new();
    if level >= 1 {
        source.push_str(&format!(
            "style parent {{ stroke: #343434 }}\nstyle first extends parent {{ fill: {} }}\n",
            color("#505050"),
        ));
        classes.push_str(" .first");
    }
    if level >= 2 {
        source.push_str(&format!("style second {{ fill: {} }}\n", color("#606060")));
        classes.push_str(" .second");
    }
    let own = if level >= 3 {
        format!(" {{ fill: {} }}", color("#707070"))
    } else {
        String::new()
    };
    source.push_str(&format!("{word} a \"Visible\" {data}{classes}{own}\n"));
    let (expected, origin) = match level {
        0 => ("#404040", format!("`style {target}`")),
        1 => ("#505050", "`.first`".into()),
        2 => ("#606060", "`.second`".into()),
        3 => ("#707070", "`a`'s own block".into()),
        4 => ("#404040", "`a`'s own block".into()),
        _ => unreachable!(),
    };
    (source, expected, origin)
}

fn fill_line(explanation: &str) -> &str {
    explanation
        .lines()
        .find(|line| line.trim_start().starts_with("fill "))
        .unwrap()
}

#[test]
fn w164_render_and_explain_agree_for_every_bpmn_xy_form_and_override_layer() {
    for &(genre, word, data) in FORMS {
        for level in 0..5 {
            let (source, expected, origin) = styled_source(genre, word, data, level);
            let analysis = bgraph::analyze_source(&source);
            assert!(
                !analysis.has_errors(),
                "{genre}/{word}/{level}: {:?}",
                analysis.diagnostics
            );
            let diagram = analysis.diagram.as_ref().unwrap();
            let drawing = analysis.layout.as_ref().unwrap();
            let node = &diagram.nodes[0];
            let def = bgraph::props::archetype(genre).unwrap();
            let resolved = Registry::build(&diagram.styles)
                .unwrap()
                .resolve_node(Style::default(), def, node)
                .unwrap();
            assert_eq!(
                resolved.fill.as_deref(),
                Some(expected),
                "{genre}/{word}/{level}"
            );
            if genre == "bpmn" {
                assert_eq!(drawing.nodes[0].style.fill.as_deref(), Some(expected));
            }
            let svg = bgraph::render_svg::render(drawing);
            assert!(
                svg.contains(expected),
                "{genre}/{word}/{level}: rendered paint missing"
            );
            let reason = bgraph::explain::explain(diagram, drawing, "a").unwrap();
            let fill = fill_line(&reason);
            assert!(
                fill.contains(expected) && fill.contains(&origin),
                "{genre}/{word}/{level}: {reason}"
            );
            assert!(
                reason.lines().any(|line| line.contains("font colour")
                    && line.contains("#202020")
                    && line.contains("`style node`")),
                "{reason}"
            );
            if level >= 1 {
                assert!(
                    reason
                        .lines()
                        .any(|line| line.trim_start().starts_with("stroke ")
                            && line.contains("#343434")
                            && line.contains("`.first`")),
                    "{reason}"
                );
            }
        }
    }
}

#[test]
fn w164_bare_bpmn_words_keep_their_notation_without_optional_data() {
    for (word, expected) in [
        ("task", Shape::Rect),
        ("node", Shape::Rect),
        ("event", Shape::Ellipse),
        ("gateway", Shape::Diamond),
        ("data", Shape::Rect),
    ] {
        let source = format!("type bpmn\n{word} a \"A\"\n");
        let analysis = bgraph::analyze_source(&source);
        assert!(!analysis.has_errors(), "{word}: {:?}", analysis.diagnostics);
        let drawing = analysis.layout.as_ref().unwrap();
        let node = &drawing.nodes[0];
        assert_eq!(
            node.style.shape,
            Some(expected),
            "bare {word} has the wrong notation"
        );
        let svg = bgraph::render_svg::render(drawing);
        match word {
            "event" => {
                assert_eq!((node.rect.w, node.rect.h), (40.0, 40.0));
                assert_eq!(node.style.stage.as_deref(), Some("intermediate"));
                assert!(svg.contains("<ellipse "));
                assert!(
                    drawing
                        .polylines
                        .iter()
                        .any(|line| line.closed && line.points.len() == 32)
                );
            }
            "gateway" => {
                assert_eq!((node.rect.w, node.rect.h), (48.0, 48.0));
                assert_eq!(node.style.gate.as_deref(), Some("exclusive"));
                assert!(svg.contains("<polygon "));
            }
            "data" => {
                assert_eq!(node.style.artifact.as_deref(), Some("object"));
                let fold = drawing
                    .polylines
                    .iter()
                    .find(|line| line.over && line.points.len() == 3)
                    .expect("an unqualified data word must draw its folded page corner");
                assert!(!fold.closed);
                assert!(svg.contains("<polyline "));
            }
            _ => assert!(svg.contains("<rect ")),
        }
    }
}

struct Lsp {
    child: Child,
    input: ChildStdin,
    output: BufReader<ChildStdout>,
    next_id: u64,
}

impl Lsp {
    fn new() -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_bgraph-lsp"))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .unwrap();
        let input = child.stdin.take().unwrap();
        let output = BufReader::new(child.stdout.take().unwrap());
        let mut server = Self {
            child,
            input,
            output,
            next_id: 1,
        };
        server.request("initialize", json!({}));
        server
    }

    fn send(&mut self, message: Value) {
        let bytes = serde_json::to_vec(&message).unwrap();
        write!(self.input, "Content-Length: {}\r\n\r\n", bytes.len()).unwrap();
        self.input.write_all(&bytes).unwrap();
        self.input.flush().unwrap();
    }

    fn read(&mut self) -> Value {
        let mut length = None;
        loop {
            let mut line = String::new();
            assert!(
                self.output.read_line(&mut line).unwrap() > 0,
                "LSP exited before replying"
            );
            if line == "\r\n" || line == "\n" {
                break;
            }
            if let Some(value) = line.strip_prefix("Content-Length:") {
                length = Some(value.trim().parse::<usize>().unwrap());
            }
        }
        let mut bytes = vec![0; length.expect("LSP frame length missing")];
        self.output.read_exact(&mut bytes).unwrap();
        serde_json::from_slice(&bytes).unwrap()
    }

    fn request(&mut self, method: &str, params: Value) -> Value {
        let id = self.next_id;
        self.next_id += 1;
        self.send(json!({"jsonrpc":"2.0", "id":id, "method":method, "params":params}));
        loop {
            let answer = self.read();
            if answer["id"] == id {
                assert!(answer.get("error").is_none(), "{answer}");
                return answer["result"].clone();
            }
        }
    }

    fn open(&mut self, source: &str) {
        self.send(json!({"jsonrpc":"2.0", "method":"textDocument/didOpen", "params":{
            "textDocument":{"uri":"file:///w164.bd", "languageId":"bgraph", "version":1, "text":source}
        }}));
    }

    fn at(source: &str, offset: usize) -> Value {
        let prefix = &source[..offset];
        let line = prefix.bytes().filter(|byte| *byte == b'\n').count();
        let character = prefix.rsplit('\n').next().unwrap().encode_utf16().count();
        json!({"textDocument":{"uri":"file:///w164.bd"}, "position":{"line":line,"character":character}})
    }
}

impl Drop for Lsp {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[test]
fn w164_real_lsp_hover_reports_the_same_winning_paint_layer() {
    let mut server = Lsp::new();
    for &(genre, word, data) in FORMS {
        for level in 0..5 {
            let (source, expected, origin) = styled_source(genre, word, data, level);
            server.open(&source);
            let offset = source.rfind(&format!("\n{word} a ")).unwrap() + 1 + word.len() + 1;
            let hover = server.request("textDocument/hover", Lsp::at(&source, offset));
            let markdown = hover["contents"]["value"].as_str().unwrap_or("");
            let fill = fill_line(markdown);
            assert!(
                fill.contains(expected) && fill.contains(&origin),
                "{genre}/{word}/{level}: {markdown}"
            );
        }
    }
}

#[test]
fn w164_real_lsp_knows_every_theme_target_and_its_property_context() {
    let mut server = Lsp::new();
    for &(genre, word, _) in FORMS {
        let target = if word == "node" {
            "node".to_string()
        } else {
            target(genre, word)
        };
        let source = format!("type {genre}\nstyle {target} {{\n  ");
        server.open(&source);
        let completion = server.request("textDocument/completion", Lsp::at(&source, source.len()));
        let labels = completion["items"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|item| item["label"].as_str())
            .collect::<Vec<_>>();
        assert!(
            labels.contains(&"shape") && labels.contains(&"fill"),
            "{genre}/{target}: {labels:?}"
        );
        assert!(
            !labels.contains(&"router") && !labels.contains(&"head"),
            "{genre}/{target}: {labels:?}"
        );
        let source = format!("type {genre}\nstyle {target} {{ fill: #404040 }}\n");
        server.open(&source);
        let offset = source.find(&format!("style {target}")).unwrap() + 6;
        let hover = server.request("textDocument/hover", Lsp::at(&source, offset));
        let markdown = hover["contents"]["value"].as_str().unwrap_or("");
        assert!(
            markdown.contains("**theme**") && markdown.contains("before classes and inline"),
            "{genre}/{target}: {markdown}"
        );
        let source = format!("type {genre}\nstyle {target} {{ head: none }}\n");
        let analysis = bgraph::analyze_source(&source);
        assert!(
            analysis
                .diagnostics
                .iter()
                .any(|diag| diag.message.contains("`head` has no effect on a node")),
            "{genre}/{target}: {:?}",
            analysis.diagnostics
        );
    }
}

#[test]
fn w164_builtin_notation_defaults_stay_below_author_layers_and_inside_bpmn() {
    let cases = [
        (
            "event",
            "stage",
            ["intermediate", "start", "end", "intermediate", "end"],
        ),
        (
            "gateway",
            "gate",
            ["exclusive", "parallel", "inclusive", "event", "complex"],
        ),
        (
            "data",
            "artifact",
            ["object", "store", "input", "output", "object"],
        ),
    ];
    for (word, property, values) in cases {
        for level in 0..5 {
            let mut source = String::from("type bpmn\n");
            if level >= 1 {
                source.push_str(&format!("style node {{ {property}: {} }}\n", values[1]));
            }
            if level >= 2 {
                source.push_str(&format!("style {word} {{ {property}: {} }}\n", values[2]));
            }
            if level >= 3 {
                source.push_str(&format!("style chosen {{ {property}: {} }}\n", values[3]));
            }
            source.push_str(&format!(
                "{word} a \"A\"{}{}\n",
                if level >= 3 { " .chosen" } else { "" },
                if level >= 4 {
                    format!(" {{ {property}: {} }}", values[4])
                } else {
                    String::new()
                },
            ));
            let analysis = bgraph::analyze_source(&source);
            assert!(
                !analysis.has_errors(),
                "{word}/{level}: {:?}",
                analysis.diagnostics
            );
            let drawing = analysis.layout.as_ref().unwrap();
            let style = &drawing.nodes[0].style;
            let actual = match property {
                "stage" => &style.stage,
                "gate" => &style.gate,
                _ => &style.artifact,
            };
            assert_eq!(actual.as_deref(), Some(values[level]), "{word}/{level}");
            let explanation =
                bgraph::explain::explain(analysis.diagram.as_ref().unwrap(), drawing, "a").unwrap();
            let line = explanation
                .lines()
                .find(|line| line.trim_start().starts_with(&format!("{property} ")))
                .unwrap();
            let origin = match level {
                0 => format!("the `{word}` form's builtin default"),
                1 => "`style node`".to_string(),
                2 => format!("`style {word}`"),
                3 => "`.chosen`".to_string(),
                _ => "`a`'s own block".to_string(),
            };
            assert!(
                line.contains(values[level]) && line.contains(&origin),
                "{explanation}"
            );
        }
    }
    for (genre, word) in [("timeline", "event"), ("xy", "scatter")] {
        let base = bgraph::style::item_defaults(
            Style::default(),
            bgraph::props::archetype(genre).unwrap(),
            word,
        );
        assert!(base.stage.is_none() && base.gate.is_none() && base.artifact.is_none());
    }
    for word in ["task", "event", "gateway", "data", "node"] {
        let base = bgraph::style::item_defaults(
            Style::default(),
            bgraph::props::archetype("bpmn").unwrap(),
            word,
        );
        assert!(base.shape.is_none() && base.router.is_none() && base.head.is_none());
        let source = format!(
            "type bpmn\nstyle author {{ shape: hexagon size: 100 70 }}\n{word} a \"A\" .author\n"
        );
        let analysis = bgraph::analyze_source(&source);
        assert!(!analysis.has_errors(), "{word}: {:?}", analysis.diagnostics);
        let drawing = analysis.layout.unwrap();
        assert_eq!(drawing.nodes[0].style.shape, Some(Shape::Hexagon), "{word}");
        assert_eq!(
            (drawing.nodes[0].rect.w, drawing.nodes[0].rect.h),
            (100.0, 70.0),
            "{word}"
        );
        assert!(bgraph::render_svg::render(&drawing).contains("<polygon "));
    }
    let analysis =
        bgraph::analyze_source("type bpmn\nstyle task { kind: user fill: #aabbcc }\nnode a\n");
    assert!(!analysis.has_errors(), "{:?}", analysis.diagnostics);
    let drawing = analysis.layout.unwrap();
    assert_eq!(drawing.nodes[0].style.task_kind.as_deref(), Some("user"));
    assert_eq!(drawing.nodes[0].style.fill.as_deref(), Some("#aabbcc"));
}

#[test]
fn w164_real_lsp_distinguishes_builtin_notation_and_equal_explicit_overrides() {
    let mut server = Lsp::new();
    for (word, property, value) in [
        ("task", "kind", "plain"),
        ("node", "kind", "plain"),
        ("event", "stage", "intermediate"),
        ("gateway", "gate", "exclusive"),
        ("data", "artifact", "object"),
    ] {
        for explicit in 0..3 {
            let canonical = target("bpmn", word);
            let source = format!(
                "type bpmn\n{}{word} a \"A\"{}\n",
                if explicit == 1 {
                    format!("style {canonical} {{ {property}: {value} }}\n")
                } else {
                    String::new()
                },
                if explicit == 2 {
                    format!(" {{ {property}: {value} }}")
                } else {
                    String::new()
                },
            );
            server.open(&source);
            let offset = source.rfind(&format!("\n{word} a ")).unwrap() + 1 + word.len() + 1;
            let hover = server.request("textDocument/hover", Lsp::at(&source, offset));
            let markdown = hover["contents"]["value"].as_str().unwrap_or("");
            let line = markdown
                .lines()
                .find(|line| line.trim_start().starts_with(&format!("{property} ")))
                .unwrap();
            let origin = match explicit {
                0 => "builtin default".to_string(),
                1 => format!("`style {canonical}`"),
                _ => "`a`'s own block".to_string(),
            };
            assert!(
                line.contains(value) && line.contains(&origin),
                "{word}/{explicit}: {markdown}"
            );
        }
    }
    for theme in bgraph::props::THEME_STYLES {
        let source = format!("type bpmn\nstyle {} {{ fill: #404040 }}\n", theme.name);
        server.open(&source);
        let hover = server.request(
            "textDocument/hover",
            Lsp::at(&source, source.find("style ").unwrap() + 6),
        );
        let markdown = hover["contents"]["value"].as_str().unwrap_or("");
        assert!(markdown.contains("**theme**"), "{}: {markdown}", theme.name);
    }
}
