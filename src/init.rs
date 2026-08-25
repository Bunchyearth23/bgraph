//! `bgraph init <genre>` — a working file to start from.
//!
//! Twenty-nine genres is more than anyone remembers, and the gap between
//! "which archetype do I want" and "what does a statement look like in it" is
//! where a reference gets opened and a diagram does not get written.
//!
//! What comes out is two things joined. The **header** is generated from the
//! archetype tables — the same ones the reference and the editor's completion
//! read — so it states the genre's positional shape in the genre's own words
//! and cannot describe a form the compiler does not accept. The **body** is the
//! genre's example file, the one the reference already reproduces and the test
//! suite already renders.
//!
//! Neither half is written twice, which is the whole point: a skeleton kept by
//! hand is a skeleton that is wrong about the keyword renamed last month.

use anyhow::{Result, bail};

use crate::props::{self, ArchetypeDef, VARIADIC};

/// Every genre that can be started, in the order the tables declare them.
pub fn genres() -> impl Iterator<Item = &'static str> {
    props::ARCHETYPES.iter().map(|a| a.name)
}

/// A complete file for `genre`, ready to render.
pub fn skeleton(genre: &str) -> Result<String> {
    let Some(def) = props::ARCHETYPES.iter().find(|a| a.name == genre) else {
        // Naming the genres beats "unknown archetype": the person asking has
        // just proved they do not know which one they want.
        let known: Vec<&str> = genres().collect();
        bail!(
            "unknown genre `{genre}`\nthe genres are: {}",
            known.join(", ")
        );
    };

    let example = crate::docgen::EXAMPLES
        .iter()
        .find(|(name, _)| *name == genre)
        .map(|(_, source)| *source)
        // `free` is the language itself and every other archetype has an
        // example, asserted by a test — so this is unreachable rather than a
        // case to handle. It still says something useful if it ever is.
        .unwrap_or("node a \"A\"\nnode b \"B\"\nlayout {\n  place a at (0, 0)\n  place b right-of a\n}\na -> b\n");

    let mut out = String::new();
    out.push_str(&header(def));
    out.push('\n');
    out.push_str(example);
    Ok(out)
}

/// The commented preamble, entirely from the tables.
fn header(def: &ArchetypeDef) -> String {
    let mut h = String::new();

    for line in wrap(def.doc, 74) {
        h.push_str("// ");
        h.push_str(&line);
        h.push('\n');
    }
    h.push_str("//\n");

    // One line per keyword, each with the shape it actually takes: a genre
    // with a second word takes a different shape under it (`D-023`), and
    // showing only the first would be showing the wrong one half the time.
    for item in def.items() {
        h.push_str("// ");
        h.push_str(&shape(def, item));
        h.push('\n');
    }

    if def.members {
        h.push_str(
            "//   …and `- name` or `+ name` on its own line inside the \
                    block, drawn\n//   as a ruled compartment under the name.\n",
        );
    }

    if !def.edge_slots.is_empty() {
        h.push_str("// ");
        h.push_str(&edge_shape(def));
        h.push('\n');
    }

    h.push_str(
        "//\n// Every setting this genre takes is in docs/reference.md, \
                under its name.\n",
    );
    h
}

/// `slice <id> ["label"] <value>` — the keyword and what rides after it.
fn shape(def: &ArchetypeDef, item: &str) -> String {
    let mut s = format!("{item} <id> [\"label\"]");
    for slot in def.slots_for(item) {
        s.push(' ');
        s.push_str(&placeholder(slot.prop, slot.arity));
    }
    s
}

fn edge_shape(def: &ArchetypeDef) -> String {
    let mut s = String::from("<id> -> <id> [\"label\"]");
    for slot in def.edge_slots {
        s.push(' ');
        s.push_str(&placeholder(slot.prop, slot.arity));
    }
    s
}

/// How a slot is written out. Arity is shown rather than summarised: an author
/// who sees `<bits> <bits>` writes two numbers, where `<bits>` alone invites
/// one and a diagnostic.
fn placeholder(prop: &str, arity: usize) -> String {
    if arity == VARIADIC {
        return format!("<{prop}>…");
    }
    let mut parts = Vec::with_capacity(arity);
    for _ in 0..arity {
        parts.push(format!("<{prop}>"));
    }
    parts.join(" ")
}

/// Greedy wrap, so a doc string written as one long line comments cleanly.
fn wrap(text: &str, width: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut current = String::new();
    for word in text.split_whitespace() {
        if !current.is_empty() && current.chars().count() + 1 + word.chars().count() > width {
            lines.push(std::mem::take(&mut current));
        }
        if !current.is_empty() {
            current.push(' ');
        }
        current.push_str(word);
    }
    if !current.is_empty() {
        lines.push(current);
    }
    lines
}
