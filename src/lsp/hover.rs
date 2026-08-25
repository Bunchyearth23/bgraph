use std::fmt::Write as _;

use serde_json::{Value, json};

use crate::props;

use super::{Block, Context, LineIndex, declared_archetype, text_of, word_at};

/// Documentation and resolved details for the word under the cursor.
pub(super) fn hover(text: &str, index: &LineIndex, offset: usize) -> Value {
    let Some((word, span)) = word_at(text, offset) else {
        return Value::Null;
    };
    let before = text[..span.start].trim_end();
    let line_start = text[..span.start].rfind('\n').map(|i| i + 1).unwrap_or(0);
    let is_key = text[span.end..].trim_start().starts_with(':');
    let ctx = Context::at(text, offset);
    let archetype = declared_archetype(text);
    let inside = crate::template::holes_in(text)
        .into_iter()
        .find(|hole| hole.span.contains(offset));

    let markdown = if let Some(hole) = inside {
        let uses = crate::template::holes_in(text)
            .iter()
            .filter(|other| other.name == hole.name)
            .count();
        let plural = if uses == 1 { "once" } else { "twice or more" };
        Some(match &hole.prefix {
            Some(prefix) => format!(
                "**hole** `{}`\n\nA field of each row of `{prefix}`. The line it sits on is written **once per row**, so the number of statements is decided by the data rather than by the file.\n\nWritten {plural} here.",
                hole.name
            ),
            None => format!(
                "**hole** `{}`\n\nBound once for the whole file.\n\nWritten {plural} here.",
                hole.name
            ),
        })
    } else if before.ends_with("type") {
        props::archetype(&word).map(|def| format!("**type** `{}`\n\n{}", def.name, def.doc))
    } else if is_key && ctx.block.is_some() {
        let table: Vec<&'static props::PropDef> = match ctx.block {
            Some(Block::Graph) => props::GRAPH_PROPS.iter().collect(),
            Some(Block::Type) => props::archetype(archetype)
                .map_or(&[][..], |def| def.props)
                .iter()
                .collect(),
            _ => props::archetype(archetype)
                .map(props::item_table)
                .unwrap_or_default(),
        };
        table.iter().find(|prop| prop.name == word).map(|prop| {
            format!(
                "**property** `{}`\n\n```\n{}\n```\n\n{}",
                prop.name, prop.syntax, prop.doc
            )
        })
    } else if before.ends_with('.') || before.ends_with("extends") {
        describe_style(text, &word)
    } else if text[line_start..span.start].trim().is_empty()
        && props::archetype(archetype).is_some_and(|def| def.accepts(&word))
        && word != "node"
    {
        props::archetype(archetype).map(|def| item_form(def, &word))
    } else if let Some((_, doc)) = props::PLACEMENTS.iter().find(|(name, _)| *name == word) {
        Some((*doc).to_string())
    } else if let Some((_, doc)) = props::KEYWORDS.iter().find(|(name, _)| *name == word) {
        Some((*doc).to_string())
    } else {
        describe_node(text, &word).or_else(|| describe_style(text, &word))
    };
    match markdown {
        Some(value) => {
            json!({ "contents": { "kind": "markdown", "value": value }, "range": index.range(span) })
        }
        None => Value::Null,
    }
}

fn item_form(def: &props::ArchetypeDef, word: &str) -> String {
    let mut form = format!("{word} <id> [\"label\"]");
    for slot in def.slots_for(word) {
        if slot.arity == props::VARIADIC {
            let _ = write!(form, " <{}>…", slot.prop);
        } else {
            for _ in 0..slot.arity {
                let _ = write!(form, " <{}>", slot.prop);
            }
        }
    }
    format!(
        "```\n{form}\n```\n\nWhat `type {}` calls its elements.\n\n{}",
        def.name, def.doc
    )
}

pub(super) fn describe_node(text: &str, id: &str) -> Option<String> {
    let diagram = crate::parser::parse(text).ok()?;
    let (i, node) = diagram
        .nodes
        .iter()
        .enumerate()
        .find(|(_, node)| node.id.as_str() == id)?;
    let mut out = format!("**node** `{id}`");
    if let Some(label) = &node.label {
        out.push_str(&format!(" — {:?}", label));
    }
    if !node.classes.is_empty() {
        out.push_str(&format!(
            "\n\nstyles: {}",
            node.classes
                .iter()
                .map(|class| format!(".{}", class.as_str()))
                .collect::<Vec<_>>()
                .join(" ")
        ));
    }
    if let Ok(layout) = crate::layout::solve(&diagram)
        && let Some(placed) = layout.nodes.get(i)
    {
        let rect = placed.rect;
        let style = &placed.style;
        let mut facts = vec![format!(
            "x {:.0}  y {:.0}   {:.0} × {:.0}",
            rect.x, rect.y, rect.w, rect.h
        )];
        if let Some(fill) = &style.fill {
            facts.push(format!("fill    {fill}"));
        }
        if let Some(stroke) = &style.stroke {
            facts.push(format!(
                "stroke  {stroke}{}",
                style
                    .stroke_width
                    .map(|width| format!("  {width}"))
                    .unwrap_or_default()
            ));
        }
        if let Some(shape) = style.shape {
            facts.push(format!("shape   {shape:?}").to_lowercase());
        }
        out.push_str(&format!("\n\n```\n{}\n```", facts.join("\n")));

        // A rectangle answers where the box ended up, but it cannot tell an
        // author which scoped declaration caused it.  Reuse the command-line
        // explanation rather than making the editor guess from source text.
        // The paint section is already represented above, so keep the
        // placement-only portion here.
        if let Ok(explanation) = crate::explain::explain(&diagram, &layout, id)
            && let Some((_, placement_and_paint)) = explanation.split_once("\n\nplacement\n")
            && let Some((placement, _)) = placement_and_paint.split_once("\npaint\n")
        {
            out.push_str(&format!(
                "\n\n**placement**\n\n```\n{}\n```",
                placement.trim()
            ));
        }
    }
    Some(out)
}

pub(super) fn describe_style(text: &str, name: &str) -> Option<String> {
    let diagram = crate::parser::parse(text).ok()?;
    let style = diagram
        .styles
        .iter()
        .find(|style| style.name.as_str() == name)?;
    let mut out = format!("**style** `{name}`");
    if let Some(parent) = &style.extends {
        out.push_str(&format!(" extends `{}`", parent.as_str()));
    }
    out.push_str("\n\n```\n");
    for property in &style.props {
        out.push_str(&format!("{}\n", text_of(text, property.span).trim()));
    }
    out.push_str("```");
    Some(out)
}
