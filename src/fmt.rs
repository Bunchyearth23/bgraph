//! A formatter for `.bd` files.
//!
//! The language has had a canonical form since the first example was written —
//! two-space indents inside a block, no trailing whitespace, at most one blank
//! line in a row — and nothing made it hold. A diagram is read far more often
//! than it is written, so the shape of the source is worth as much as the
//! shape of the picture.
//!
//! **Interior alignment is left exactly as written.** The `packet` example
//! lines its bit ranges up in columns and that is a real style, not an
//! accident; a formatter that crushed it would be arguing with the author
//! about something the grammar has no opinion on. Indentation is structural
//! and is therefore the formatter's business; column alignment is taste and is
//! not.
//!
//! It is **lexical on purpose**, like the nesting guard and the template
//! scanner. Formatting through the AST would drop every comment and every
//! blank line, which are the two things an author uses to say what the
//! grammar cannot — and a formatter that quietly deletes your notes is one
//! nobody runs twice.

/// Reformat a source, or report why it will not.
///
/// Refuses a file that does not parse rather than reflowing something it does
/// not understand: an unbalanced brace reindents everything after it, and the
/// diff would bury the one line that was wrong.
pub fn format(source: &str) -> anyhow::Result<String> {
    crate::parser::parse_diagram(source)?;
    Ok(reflow(source))
}

/// The formatting itself, on text that is known to parse.
fn reflow(source: &str) -> String {
    let mut out = String::with_capacity(source.len());
    let mut depth: usize = 0;
    let mut blanks = 0usize;

    for raw in source.lines() {
        let line = raw.trim();

        if line.is_empty() {
            // At most one blank line in a row, and none at the top of a block:
            // a run of them is a paragraph break someone stopped using.
            blanks += 1;
            continue;
        }

        // A closing brace belongs at the indent of the line that opened it —
        // which is one *less* than the depth we are still nominally at. That
        // is a display decision only: the running depth comes from the braces
        // the line actually holds, counted once.
        let closes_first = line.starts_with('}');
        let indent = if closes_first {
            depth.saturating_sub(1)
        } else {
            depth
        };

        if blanks > 0 && !out.is_empty() && !closes_first {
            out.push('\n');
        }
        blanks = 0;

        for _ in 0..indent {
            out.push_str("  ");
        }
        out.push_str(line.trim_end());
        out.push('\n');

        // Braces inside a string or after a `//` are text, so they are counted
        // by the same scan that decides where the line goes.
        depth = (depth as isize + net_braces(line)).max(0) as usize;
    }

    out
}

/// Rewrite `opacity` as the fourth channel of the colours it applied to.
///
/// `D-016` removed the property: transparency belongs to the paint, which is
/// what lets a shape keep an opaque border over a translucent fill. That was
/// the right call and it is also a breaking change — `opacity: 0.5` fails with
/// `unknown property` and nothing moved the files that had one (`X-008`).
///
/// This is that migration. It is surgical rather than a reprint: the alpha
/// digits are inserted into the colour where it stands and the `opacity` line
/// is cut out, so comments, blank lines and hand-aligned columns survive
/// untouched — the same discipline `relayout.rs` follows for the same reason.
///
/// **It declines rather than guesses.** A block carrying an `opacity` and no
/// colour to carry the alpha is an error naming the line, because a migration
/// that invents a `#ffffff` has changed the drawing while claiming not to. So
/// is a named colour: `white` has no fourth channel, and turning it into a hex
/// literal is the author's decision to make, not this function's.
pub fn migrate(source: &str) -> anyhow::Result<String> {
    let diagram = crate::parser::parse_diagram(source)?;

    let decls = declarations(source);
    let mut edits: Vec<(usize, usize, String)> = Vec::new();

    for block in blocks_with_opacity(&decls) {
        let here = |d: &&Decl| d.block == block;
        let fades: Vec<&Decl> = decls
            .iter()
            .filter(|d| here(d) && d.name == OPACITY)
            .collect();
        let paints: Vec<&Decl> = decls
            .iter()
            .filter(|d| here(d) && PAINTS.contains(&d.name.as_str()))
            .collect();

        // The last one wins, the way a repeated property does everywhere else.
        let last = fades.last().expect("the block was chosen for having one");
        let factor = fraction(source, last)?;

        if paints.is_empty() {
            anyhow::bail!(
                "line {}: `opacity` has nothing to ride on — the block declares no \
                 `fill`, `stroke` or `font-color`. Add the colour it was fading, or \
                 delete the line",
                line_of(source, last.start)
            );
        }

        for paint in &paints {
            let Some((s, e)) = colour_in(source, paint) else {
                continue;
            };
            let written = &source[s..e];
            if !written.starts_with('#') {
                anyhow::bail!(
                    "line {}: `opacity` applies to the named colour `{written}`, which has \
                     no fourth channel. Write it as `#rrggbb` and run this again — turning \
                     a name into a number is your call, not this command's",
                    line_of(source, s)
                );
            }
            edits.push((s, e, fade(written, factor)));
        }

        for fade in fades {
            let (s, e) = cut(source, fade.start, fade.end);
            let joint = s > 0
                && e < source.len()
                && source.as_bytes()[s - 1] != b'\n'
                && source.as_bytes()[e] != b'\n';
            edits.push((s, e, if joint { " ".into() } else { String::new() }));
        }
    }

    // v2 has one home for a box's geometry: a scoped `layout` block. The
    // parser retained each legacy clause's exact byte range, so this rewrite
    // moves the relation without reprinting its numbers, whitespace or
    // comments. A local member stays local; a frame is placed by its parent.
    let mut moved: Vec<MovedPlacement<'_>> = Vec::new();
    for node in &diagram.nodes {
        if let (Some(_), Some(span)) = (&node.placement, node.placement_span) {
            moved.push(MovedPlacement {
                target: node.id.as_str(),
                span,
                scope: owner_of(&diagram, node.id.as_str()),
            });
        }
    }
    for group in &diagram.groups {
        if let (Some(_), Some(span)) = (&group.placement, group.placement_span) {
            moved.push(MovedPlacement {
                target: group.id.as_str(),
                span,
                scope: owner_of(&diagram, group.id.as_str()),
            });
        }
    }
    for placing in &diagram.uses {
        if let (Some(_), Some(span)) = (&placing.placement, placing.placement_span) {
            moved.push(MovedPlacement {
                target: placing.prefix.as_str(),
                span,
                scope: None,
            });
        }
    }
    // A placement migration is allowed only to preserve the picture. If the
    // source itself does not render, there is no honest baseline to compare,
    // so do not write a plausible-looking but unverified v2 file.
    let before_picture = if moved.is_empty() {
        None
    } else {
        Some(crate::render_source(source).map_err(|error| {
            anyhow::anyhow!(
                "cannot verify the inline-placement migration because the source does not render: {error}"
            )
        })?)
    };
    migrate_placements(source, &diagram, &moved, &mut edits);

    // Right to left, so an earlier edit's offsets are still the ones measured.
    edits.sort_by_key(|(s, _, _)| *s);
    let mut out = source.to_string();
    for (s, e, with) in edits.into_iter().rev() {
        out.replace_range(s..e, &with);
    }
    crate::parser::parse_diagram(&out)?;
    if let Some(before_picture) = before_picture {
        let after_picture = crate::render_source(&out)?;
        if after_picture != before_picture {
            anyhow::bail!(
                "the placement migration would change the rendered SVG, so it left the source untouched"
            );
        }
    }
    Ok(out)
}

struct MovedPlacement<'a> {
    target: &'a str,
    span: crate::ast::Span,
    /// `None` is the root layout; `Some` is the owning group's layout.
    scope: Option<&'a str>,
}

/// The direct group that owns an entity. Group membership is already the
/// canonical scoping relation used by the validator and solver.
fn owner_of<'a>(diagram: &'a crate::ast::Diagram, name: &str) -> Option<&'a str> {
    diagram
        .groups
        .iter()
        .find(|group| group.members.iter().any(|member| member.as_str() == name))
        .map(|group| group.id.as_str())
}

/// Move every inline placement into its one canonical layout scope.
fn migrate_placements(
    source: &str,
    diagram: &crate::ast::Diagram,
    moved: &[MovedPlacement<'_>],
    edits: &mut Vec<(usize, usize, String)>,
) {
    use std::collections::BTreeMap;

    let mut by_scope: BTreeMap<Option<&str>, Vec<&MovedPlacement<'_>>> = BTreeMap::new();
    for placement in moved {
        let (start, end) = cut(source, placement.span.start, placement.span.end);
        let joint = start > 0
            && end < source.len()
            && source.as_bytes()[start - 1] != b'\n'
            && source.as_bytes()[end] != b'\n';
        edits.push((start, end, if joint { " ".into() } else { String::new() }));
        by_scope.entry(placement.scope).or_default().push(placement);
    }

    for (scope, placements) in by_scope {
        let statements = placements
            .iter()
            .map(|placement| {
                format!(
                    "place {} {}",
                    placement.target,
                    source[placement.span.start..placement.span.end].trim()
                )
            })
            .collect::<Vec<_>>();

        let existing = diagram
            .layouts
            .iter()
            .rev()
            .find(|layout| layout.scope.as_ref().map(|name| name.as_str()) == scope);
        if let Some(layout) = existing {
            let at = layout.span.end.saturating_sub(1);
            let indent = format!("{}  ", line_indent(source, layout.span.start));
            let text = statements
                .iter()
                .map(|statement| format!("\n{indent}{statement}"))
                .collect::<String>();
            edits.push((at, at, text));
            continue;
        }

        match scope {
            None => {
                let prefix = if source.trim_end().is_empty() {
                    ""
                } else {
                    "\n"
                };
                let text = format!("{prefix}\nlayout {{\n  {}\n}}\n", statements.join("\n  "));
                edits.push((source.len(), source.len(), text));
            }
            Some(scope) => {
                let Some(group) = diagram
                    .groups
                    .iter()
                    .find(|group| group.id.as_str() == scope)
                else {
                    continue;
                };
                let at = group.span.end.saturating_sub(1);
                let group_indent = line_indent(source, group.span.start);
                let indent = format!("{group_indent}  ");
                let body = statements
                    .iter()
                    .map(|statement| format!("{indent}  {statement}"))
                    .collect::<Vec<_>>()
                    .join("\n");
                edits.push((
                    at,
                    at,
                    format!("\n{indent}layout {{\n{body}\n{indent}}}\n{group_indent}"),
                ));
            }
        }
    }
}

fn line_indent(source: &str, offset: usize) -> String {
    let start = source[..offset.min(source.len())]
        .rfind('\n')
        .map_or(0, |newline| newline + 1);
    source[start..offset.min(source.len())]
        .chars()
        .take_while(|character| matches!(character, ' ' | '\t'))
        .collect()
}

const OPACITY: &str = "opacity";
/// Every property whose value is a paint, aliases included — an alias is the
/// same property under a second word, so missing one would migrate half a file.
const PAINTS: &[&str] = &[
    "fill",
    "background",
    "stroke",
    "border",
    "font-color",
    "text-color",
];

/// Which blocks carry an `opacity`, in the order they appear, each once.
fn blocks_with_opacity(decls: &[Decl]) -> Vec<usize> {
    let mut out: Vec<usize> = Vec::new();
    for d in decls.iter().filter(|d| d.name == OPACITY) {
        if !out.contains(&d.block) {
            out.push(d.block);
        }
    }
    out
}

/// The colour among a property's values.
///
/// `stroke: #334155 1 dashed` takes its three parts **in any order**, so
/// position cannot say which is which — the colour is the `#` literal, or
/// failing that the one word that is neither a width nor a dash pattern.
fn colour_in(source: &str, decl: &Decl) -> Option<(usize, usize)> {
    let word = |(s, e): &(usize, usize)| &source[*s..*e];
    decl.values
        .iter()
        .find(|v| word(v).starts_with('#'))
        .or_else(|| {
            decl.values.iter().find(|v| {
                let w = word(v);
                !matches!(w, "solid" | "dashed" | "dotted") && w.parse::<f64>().is_err()
            })
        })
        .copied()
}

/// The declared fraction, refused if it is not one.
fn fraction(source: &str, decl: &Decl) -> anyhow::Result<f64> {
    let value = decl
        .values
        .first()
        .map(|(s, e)| &source[*s..*e])
        .unwrap_or("");
    match value.parse::<f64>() {
        Ok(v) if (0.0..=1.0).contains(&v) => Ok(v),
        _ => anyhow::bail!(
            "line {}: `opacity: {value}` is not a fraction between 0 and 1, so there is \
             no alpha to move",
            line_of(source, decl.start)
        ),
    }
}

/// A colour with `factor` folded into its alpha.
///
/// The existing alpha is multiplied rather than replaced: an element opacity
/// and a paint alpha composited, so a colour that already carried one keeps
/// what it meant. Fully opaque is written as no alpha at all, which is what
/// `render_svg::split_alpha` reads back and what the reference shows.
fn fade(colour: &str, factor: f64) -> String {
    let (rgb, alpha) = crate::render_svg::split_alpha(colour);
    let a = alpha.unwrap_or(1.0) * factor;
    if a >= 1.0 {
        return rgb;
    }
    format!("{rgb}{:02x}", (a * 255.0).round().clamp(0.0, 255.0) as u8)
}

/// The span to remove for a declaration, widened over the spaces around it.
///
/// A declaration alone on its line takes the line with it; one sharing a line
/// leaves its neighbours a single space. Without this a cut inside a block
/// leaves either a double space or a blank line that was never in the file.
fn cut(source: &str, start: usize, end: usize) -> (usize, usize) {
    let b = source.as_bytes();
    let mut s = start;
    while s > 0 && matches!(b[s - 1], b' ' | b'\t') {
        s -= 1;
    }
    let mut e = end;
    while e < b.len() && matches!(b[e], b' ' | b'\t') {
        e += 1;
    }
    let alone = (s == 0 || b[s - 1] == b'\n') && (e == b.len() || b[e] == b'\n');
    if alone && e < b.len() {
        e += 1;
    }
    (s, e)
}

fn line_of(source: &str, offset: usize) -> usize {
    source[..offset].bytes().filter(|c| *c == b'\n').count() + 1
}

/// One `name: value…` found by the scan, and which block it belongs to.
struct Decl {
    name: String,
    block: usize,
    start: usize,
    end: usize,
    values: Vec<(usize, usize)>,
}

/// Every property declaration in the source, tagged with its block.
///
/// Lexical, like everything else in this file: the migration has to survive
/// comments and hand alignment, and an AST has already thrown both away.
fn declarations(source: &str) -> Vec<Decl> {
    let toks = tokens(source);
    let mut out = Vec::new();
    let mut stack: Vec<usize> = Vec::new();
    let mut opened = 0usize;
    let mut i = 0;

    while i < toks.len() {
        match toks[i].text {
            "{" => {
                opened += 1;
                stack.push(opened);
                i += 1;
            }
            "}" => {
                stack.pop();
                i += 1;
            }
            ":" => i += 1,
            name if toks.get(i + 1).map(|t| t.text) == Some(":") => {
                let mut values: Vec<(usize, usize)> = Vec::new();
                let mut j = i + 2;
                while let Some(v) = toks.get(j) {
                    // A value runs until the next `name:` pair or a brace —
                    // which is what lets `stroke: #334155 1` keep both parts.
                    if matches!(v.text, "{" | "}") || toks.get(j + 1).map(|t| t.text) == Some(":") {
                        break;
                    }
                    values.push((v.start, v.end));
                    j += 1;
                }
                out.push(Decl {
                    name: name.to_string(),
                    block: stack.last().copied().unwrap_or(0),
                    start: toks[i].start,
                    end: values.last().map_or(toks[i + 1].end, |(_, e)| *e),
                    values,
                });
                i = j;
            }
            _ => i += 1,
        }
    }

    out
}

struct Tok<'a> {
    text: &'a str,
    start: usize,
    end: usize,
}

/// Words, strings, braces and colons — comments dropped, string interiors kept
/// whole, so a `#` in a label can never be mistaken for a colour.
fn tokens(source: &str) -> Vec<Tok<'_>> {
    let b = source.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;

    while i < b.len() {
        if b[i].is_ascii_whitespace() {
            i += 1;
        } else if b[i] == b'/' && b.get(i + 1) == Some(&b'/') {
            while i < b.len() && b[i] != b'\n' {
                i += 1;
            }
        } else if b[i] == b'"' {
            let start = i;
            i += 1;
            while i < b.len() {
                match b[i] {
                    b'\\' => i += 2,
                    b'"' => {
                        i += 1;
                        break;
                    }
                    _ => i += 1,
                }
            }
            let end = i.min(b.len());
            out.push(Tok {
                text: &source[start..end],
                start,
                end,
            });
        } else if matches!(b[i], b'{' | b'}' | b':') {
            out.push(Tok {
                text: &source[i..i + 1],
                start: i,
                end: i + 1,
            });
            i += 1;
        } else {
            let start = i;
            // Only ASCII delimiters end a word, so a multi-byte character can
            // never be cut in half by this scan.
            while i < b.len()
                && !b[i].is_ascii_whitespace()
                && !matches!(b[i], b'{' | b'}' | b':' | b'"')
                && !(b[i] == b'/' && b.get(i + 1) == Some(&b'/'))
            {
                i += 1;
            }
            out.push(Tok {
                text: &source[start..i],
                start,
                end: i,
            });
        }
    }

    out
}

/// How many braces the line leaves open, ignoring any inside a string or after
/// a `//`.
fn net_braces(line: &str) -> isize {
    let mut net = 0isize;
    let mut in_string = false;
    let mut chars = line.chars().peekable();

    while let Some(c) = chars.next() {
        match c {
            '\\' if in_string => {
                chars.next();
            }
            '"' => in_string = !in_string,
            '/' if !in_string && chars.peek() == Some(&'/') => break,
            '{' if !in_string => net += 1,
            '}' if !in_string => net -= 1,
            _ => {}
        }
    }

    net
}
