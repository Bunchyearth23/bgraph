use serde_json::{Value, json};

use crate::props::{self, Ctx};

use super::{
    Block, Context, Server, completion, declared, declared_archetype, declared_items,
    is_ident_char, is_rel_dir, item_snippet, snippet,
};

impl Server {
    pub(super) fn completion(&self, params: &Value) -> Value {
        let Some((text, _index, offset)) = self.doc(params) else {
            return json!({ "isIncomplete": false, "items": [] });
        };
        completion::response(text, offset, |line| self.items_for(text, line, offset))
    }

    /// Completion is deliberately quiet: it stays out of labels and comments,
    /// hides aliases, and only fires on `.` where a class reference can go.
    /// A suggestion that appears while you are typing prose is worse than none.
    pub(super) fn items_for(&self, text: &str, line: &str, offset: usize) -> Vec<Value> {
        let ctx = Context::at(text, offset);
        if ctx.in_string || ctx.in_comment {
            return Vec::new();
        }

        if ctx.block == Some(Block::Layout) {
            return layout_items(text, line, declared_archetype(text));
        }

        let trimmed = line.trim_start();
        let archetype = declared_archetype(text);

        // Inside `{{ … }}`: offer the holes this file already uses. A template
        // is written by repeating its own parameter names, and misspelling one
        // silently produces a second parameter nobody binds.
        if let Some(open) = line.rfind("{{")
            && !line[open..].contains("}}")
        {
            let typed = line[open + 2..].trim_start();
            let mut names: Vec<String> = Vec::new();
            for hole in crate::template::holes_in(text) {
                if !names.contains(&hole.name) {
                    names.push(hole.name);
                }
            }
            return names
                .into_iter()
                .filter(|n| n.starts_with(typed) || typed.is_empty())
                .map(|name| {
                    let doc = match name.split_once('.') {
                        Some((prefix, field)) => format!(
                            "`{field}` of each row of `{prefix}`. This line is written once \
                         per row."
                        ),
                        None => format!("A value bound once for the whole file: `{name}`."),
                    };
                    completion(&name, 6, "hole", &doc)
                })
                .collect();
        }

        // `.class` — only after whitespace, so `1.5` and `..>` stay quiet.
        if let Some(dot) = line.rfind('.')
            && line[dot + 1..].chars().all(is_ident_char)
            && line[..dot]
                .chars()
                .next_back()
                .is_none_or(char::is_whitespace)
        {
            return declared(text, "style")
                .into_iter()
                .map(|(name, _)| {
                    completion(&name, 7, "style", &format!("Apply the `{name}` style."))
                })
                .collect();
        }

        // Properties a container block also accepts, held back until the
        // statement level has had its say. Empty everywhere else.
        let mut also: Vec<Value> = Vec::new();

        if let Some(block) = ctx.block {
            // An item block sees the archetype's own vocabulary first, so
            // `kind:` offers state kinds in a machine and element kinds in an
            // architecture view — never both.
            let items: Vec<&'static props::PropDef> = props::archetype(archetype)
                .map(props::item_table)
                .unwrap_or_default();
            let table: Vec<&'static props::PropDef> = match block {
                Block::Graph => props::GRAPH_PROPS.iter().collect(),
                Block::Type => props::archetype(archetype)
                    .map_or(&[][..], |a| a.props)
                    .iter()
                    .collect(),
                _ => items,
            };

            // `name: <here>` — complete the value, if the property enumerates.
            if let Some(colon) = line.rfind(':') {
                let key: String = line[..colon]
                    .chars()
                    .rev()
                    .take_while(|c| is_ident_char(*c))
                    .collect::<Vec<_>>()
                    .into_iter()
                    .rev()
                    .collect();

                return table
                    .iter()
                    .find(|p| p.name == key)
                    .map(|def| {
                        def.words
                            .iter()
                            .map(|w| completion(w, 20, def.syntax, def.doc))
                            .collect()
                    })
                    .unwrap_or_default();
            }

            let want = match block {
                Block::Node | Block::Container => Some(Ctx::Node),
                Block::Edge => Some(Ctx::Edge),
                // A theme reads a named part of the vocabulary and
                // `check_styles` warns when a style reaches outside it;
                // offering what it does not read is offering that warning.
                Block::Style(name) => name.and_then(props::theme_style).and_then(|t| t.ctx),
                _ => None,
            };

            let only: &[&str] = match block {
                Block::Style(name) => name
                    .and_then(props::theme_style)
                    .map_or(&[][..], |t| t.only),
                _ => &[],
            };

            let offered: Vec<Value> = table
                .iter()
                .filter(|p| !p.alias)
                .filter(|p| want.is_none_or(|c| p.ctx.allows(c)))
                .filter(|p| only.is_empty() || only.contains(&p.name))
                .enumerate()
                .map(|(i, p)| {
                    let mut it = completion(p.name, 10, p.syntax, p.doc);
                    it["insertText"] = json!(format!("{}: ", p.name));
                    // Table order is meaningful; keep it instead of alphabetical.
                    // Sorted after the statement words in a container, where
                    // both are offered and declaring is the commoner act.
                    it["sortText"] = json!(format!("z{i:03}"));
                    it
                })
                .collect();

            // A container holds statements too, so it does not return here —
            // it carries its properties down to the statement level and the
            // two lists are offered together.
            if block != Block::Container {
                return offered;
            }
            also = offered;
        }

        // ------------------------------------------------ statement level

        let words: Vec<&str> = trimmed.split_whitespace().collect();
        let ends_with_space = line.ends_with(char::is_whitespace);
        // The word being completed, or "" when a fresh one is starting.
        let current = if ends_with_space {
            ""
        } else {
            words.last().copied().unwrap_or("")
        };
        let preceding = if ends_with_space {
            words.last().copied().unwrap_or("")
        } else if words.len() >= 2 {
            words[words.len() - 2]
        } else {
            ""
        };

        let item = props::archetype(archetype).map_or("node", |a| a.item);

        if preceding == "type" {
            return props::ARCHETYPES
                .iter()
                .map(|a| completion(a.name, 20, "archetype", a.doc))
                .collect();
        }

        if is_rel_dir(preceding) {
            return declared_items(text, archetype)
                .into_iter()
                .map(|(name, label)| completion(&name, 6, item, &label))
                .collect();
        }

        if preceding == "align" {
            return props::ALIGNMENTS
                .iter()
                .map(|a| completion(a, 20, "align", "Cross-axis alignment against the anchor."))
                .collect();
        }

        // `style <here>` — the names the renderer looks up by itself, and the
        // genre's own item word. A class can be called anything, which is
        // exactly why the handful of names that are *not* free had to be
        // offered: `style grid` looked like a typo and a typo looked like a
        // theme (`W-134`).
        if preceding == "style" {
            let mut out: Vec<Value> = props::THEME_STYLES
                .iter()
                .map(|t| completion(t.name, 7, "theme", t.doc))
                .collect();
            if let Some(def) = props::archetype(archetype) {
                for (i, word) in def.items().enumerate() {
                    out.insert(
                        i,
                        completion(
                            word,
                            7,
                            "theme",
                            &format!(
                                "Every `{word}` on the diagram, without being referenced — \
                             what `type {archetype}` calls its items."
                            ),
                        ),
                    );
                }
            }
            return out;
        }

        if preceding == "extends" {
            return declared(text, "style")
                .into_iter()
                .map(|(name, _)| completion(&name, 7, "style", &format!("Inherit from `{name}`.")))
                .collect();
        }

        // `style name …` — `extends` is the only word that may follow a name,
        // and it was reachable only by knowing it was there.
        if words.first().is_some_and(|w| *w == "style")
            && words.len() >= 2
            && preceding != "style"
            && !words.contains(&"extends")
        {
            return vec![snippet(
                "extends",
                14,
                "extends <style>",
                "Inherit every property of another style, then override what differs.",
                "extends ${1:parent}$0",
            )];
        }

        // Placements only make sense where placement is honoured, and only
        // once the item already has an identifier.
        if words.first().is_some_and(|w| *w == item || *w == "node")
            && words.len() >= 2
            && archetype == "free"
        {
            return props::PLACEMENTS
                .iter()
                .map(|(name, doc)| completion(name, 14, "placement", doc))
                .collect();
        }

        // The first word of a statement: a keyword, or a node id starting an
        // edge. A line holding nothing but indentation is that position too —
        // asking on a blank line used to answer nothing, which is the one
        // moment you most want the list.
        if words.is_empty() || (words.len() == 1 && !ends_with_space) {
            let mut out: Vec<Value> = props::KEYWORDS
                .iter()
                .map(|(name, doc)| {
                    // The shape each statement takes, so accepting one leaves
                    // a well-formed line rather than a bare word.
                    let insert = match *name {
                        "type" => "type ${1:archetype}$0".to_string(),
                        "graph" => "graph { $0 }".to_string(),
                        "style" => "style ${1:name} {\n  $0\n}".to_string(),
                        "group" => "group ${1:id} \"${2:label}\" {\n  $0\n}".to_string(),
                        "activate" => "activate ${1:participant} {\n  $0\n}".to_string(),
                        other => format!("{other} ${{1:id}} \"${{2:label}}\"$0"),
                    };
                    snippet(name, 14, "keyword", doc, &insert)
                })
                .collect();

            // The archetype's own word first; `node` stays available. A second
            // word carries a second positional shape (`D-023`), so leaving it
            // out would hide half of what the archetype accepts — and its own
            // doc is what tells the two apart, so it is offered with that
            // rather than a generic line.
            if let Some(def) = props::archetype(archetype) {
                let mut at = 0;
                if def.item != "node" {
                    out.insert(
                        at,
                        snippet(
                            def.item,
                            14,
                            &format!("{} <id> [\"label\"] …", def.item),
                            &format!(
                                "Declare a {} — what `type {archetype}` calls its elements.",
                                def.item
                            ),
                            &item_snippet(def.item, def.slots),
                        ),
                    );
                    at += 1;
                }
                for alt in def.alt_items {
                    out.insert(
                        at,
                        snippet(
                            alt.item,
                            14,
                            &format!("{} <id> [\"label\"] …", alt.item),
                            alt.doc,
                            &item_snippet(alt.item, alt.slots),
                        ),
                    );
                    at += 1;
                }
            }
            if archetype == "free" && !current.is_empty() {
                out.extend(
                    declared_items(text, archetype)
                        .into_iter()
                        .map(|(name, label)| completion(&name, 6, item, &label)),
                );
            }
            out.extend(also);
            return out;
        }

        Vec::new()
    }
}

/// Completion inside the canonical geometry block.  It deliberately does not
/// offer declarations or style properties: a `layout` says how already named
/// entities relate, never what new entities exist.
fn layout_items(text: &str, line: &str, archetype: &str) -> Vec<Value> {
    if !matches!(
        props::archetype(archetype).map(|def| def.layout_capability()),
        Some(props::LayoutCapability::Declared | props::LayoutCapability::Hybrid)
    ) {
        return Vec::new();
    }

    let words: Vec<&str> = line.split_whitespace().collect();
    let ends_with_space = line.ends_with(char::is_whitespace);
    let preceding = if ends_with_space {
        words.last().copied().unwrap_or("")
    } else if words.len() >= 2 {
        words[words.len() - 2]
    } else {
        ""
    };
    let entities = || {
        declared_items(text, archetype)
            .into_iter()
            .map(|(name, label)| completion(&name, 6, "entity", &label))
            .collect()
    };

    if words.is_empty() || (words.len() == 1 && !ends_with_space) {
        return vec![
            snippet(
                "place",
                14,
                "place <target> <position>",
                "Place one declared entity. Coordinates and relative anchors live only here.",
                "place ${1:target} at (${2:x}, ${3:y})$0",
            ),
            snippet(
                "align",
                14,
                "align <axis> <members…>",
                "Align declared nodes on an explicit axis.",
                "align ${1:top} ${2:first} ${3:other}$0",
            ),
            snippet(
                "equal-width",
                14,
                "equal-width <members…>",
                "Make declared nodes as wide as their widest member.",
                "equal-width ${1:first} ${2:other}$0",
            ),
            snippet(
                "equal-height",
                14,
                "equal-height <members…>",
                "Make declared nodes as tall as their tallest member.",
                "equal-height ${1:first} ${2:other}$0",
            ),
            snippet(
                "row",
                14,
                "row <members…> [gap <n>]",
                "Place members left to right in the written order.",
                "row ${1:first} ${2:next} gap ${3:32}$0",
            ),
            snippet(
                "column",
                14,
                "column <members…> [gap <n>]",
                "Place members top to bottom in the written order.",
                "column ${1:first} ${2:next} gap ${3:32}$0",
            ),
        ];
    }

    match words[0] {
        "place" if words.len() == 1 || preceding == "place" => entities(),
        "place" if words.len() == 2 && ends_with_space => props::PLACEMENTS
            .iter()
            .map(|(name, doc)| completion(name, 14, "position", doc))
            .collect(),
        "place" if is_rel_dir(preceding) => entities(),
        "place" if preceding == "align" => props::ALIGNMENTS
            .iter()
            .map(|axis| {
                completion(
                    axis,
                    20,
                    "align",
                    "Cross-axis alignment against the anchor.",
                )
            })
            .collect(),
        "align" if words.len() == 1 || preceding == "align" => {
            ["top", "bottom", "left", "right", "center-x", "center-y"]
                .into_iter()
                .map(|axis| {
                    completion(
                        axis,
                        20,
                        "axis",
                        "The edge or centre line shared by the members.",
                    )
                })
                .collect()
        }
        "align" | "equal-width" | "equal-height" | "row" | "column" => entities(),
        _ => Vec::new(),
    }
}
