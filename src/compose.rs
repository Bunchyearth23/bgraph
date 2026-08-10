//! Resolving `include`, so that by the time anything is checked there is one
//! diagram and no such thing as a file boundary (`D-029`).
//!
//! # How the spans stay honest
//!
//! Every declaration carries a byte range, and the language server reports
//! diagnostics and jumps to definitions from them. Once two files are merged
//! those ranges would be ambiguous — byte 40 of *which* text?
//!
//! Rather than give every span a file identity, which would touch every
//! construction of one in the crate, the files are laid end to end into a
//! single buffer and each file's spans are shifted by where its text starts.
//! One offset then answers both questions at once: [`SourceMap::locate`] takes
//! any span back to a file, a line and a column. Nothing else in the crate
//! learns that composition exists.
//!
//! # What is refused
//!
//! - A cycle, naming the chain rather than recursing into it.
//! - A name declared twice with no prefix to separate the two, naming both
//!   files. That is `D-005` — unknown names are hard errors — applied to a
//!   second file rather than a second spelling.
//! - Anything but a `style` in a `.bds`. The extension promises a shared
//!   palette, and a file that could also declare nodes would be a `.bd` under
//!   a misleading name.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

use crate::ast::{Diagram, Prop, Span, Spanned};

/// Where every byte of the composed buffer came from.
#[derive(Debug)]
pub struct SourceMap {
    /// The files laid end to end — what every span in the composed diagram
    /// indexes into.
    pub text: String,
    /// Each file's starting offset, in the order they were laid down.
    files: Vec<(usize, PathBuf)>,
}

impl SourceMap {
    /// The file, line and column a byte offset falls in. Lines and columns are
    /// zero-based, as [`crate::lsp::LineIndex`] reports them.
    pub fn locate(&self, offset: usize) -> (&Path, usize, usize) {
        let i = self
            .files
            .partition_point(|(start, _)| *start <= offset)
            .saturating_sub(1);
        let (start, path) = &self.files[i];
        let local = &self.text[*start..offset.min(self.text.len())];
        let line = local.matches('\n').count();
        let col = local.rsplit('\n').next().map_or(0, |l| l.chars().count());
        (path, line, col)
    }

    /// The one file's worth of text a span belongs to, for showing the line a
    /// diagnostic points at.
    pub fn source_of(&self, offset: usize) -> &str {
        let i = self
            .files
            .partition_point(|(start, _)| *start <= offset)
            .saturating_sub(1);
        let start = self.files[i].0;
        let end = self
            .files
            .get(i + 1)
            .map_or(self.text.len(), |(next, _)| *next);
        &self.text[start..end]
    }
}

/// A file and everything it pulls in, as one diagram.
#[derive(Debug)]
pub struct Composed {
    /// Every file's declarations, merged into one.
    pub diagram: Diagram,
    /// What takes a span in that diagram back to the file it was written in.
    pub map: SourceMap,
}

/// Load `path`, resolving its includes.
pub fn load(path: &Path) -> Result<Composed> {
    let mut state = State {
        diagram: Diagram::default(),
        text: String::new(),
        files: Vec::new(),
        // Where each declared name came from, so a clash can name both files
        // rather than only the second.
        origin: HashMap::new(),
        chain: Vec::new(),
    };
    state.absorb(path, None)?;

    Ok(Composed {
        diagram: state.diagram,
        map: SourceMap {
            text: state.text,
            files: state.files,
        },
    })
}

struct State {
    diagram: Diagram,
    text: String,
    files: Vec<(usize, PathBuf)>,
    origin: HashMap<String, PathBuf>,
    chain: Vec<PathBuf>,
}

impl State {
    fn absorb(&mut self, path: &Path, prefix: Option<&str>) -> Result<()> {
        let path = path
            .canonicalize()
            .with_context(|| format!("cannot open {}", path.display()))?;

        if self.chain.contains(&path) {
            let mut chain: Vec<String> =
                self.chain.iter().map(|p| p.display().to_string()).collect();
            chain.push(path.display().to_string());
            bail!("`include` runs in a circle: {}", chain.join(" → "));
        }

        let source = std::fs::read_to_string(&path)
            .with_context(|| format!("cannot read {}", path.display()))?;
        let mut diagram = crate::parser::parse(&source)
            .map_err(|e| anyhow::anyhow!("{}: {}", path.display(), e.message))?;

        // A `.bds` is styles and nothing else. Checked here rather than in
        // `check`, because by the time `check` runs the file boundary is gone.
        if path.extension().is_some_and(|e| e == "bds") {
            let stray = if !diagram.nodes.is_empty() {
                Some("an item")
            } else if !diagram.edges.is_empty() {
                Some("an edge")
            } else if !diagram.groups.is_empty() {
                Some("a group")
            } else if diagram.archetype.is_some() {
                Some("a `type`")
            } else if !diagram.graph.is_empty() {
                Some("a `graph` block")
            } else {
                None
            };
            if let Some(what) = stray {
                bail!(
                    "{} is a `.bds`, which holds `style` declarations only — \
                     it declares {what}. Rename it `.bd` if it is a diagram",
                    path.display()
                );
            }
        }

        // Depth first, so an included file's text — and its spans — land
        // before the text of the file that asked for it. The order does not
        // matter to the reader; what matters is that it is the same order the
        // offsets are computed in.
        let includes = std::mem::take(&mut diagram.includes);
        self.chain.push(path.clone());
        for include in &includes {
            let target = path
                .parent()
                .unwrap_or_else(|| Path::new("."))
                .join(include.path.as_str());
            // A prefix inside a prefix compounds, so `a.b.c` reads the way it
            // looks rather than silently flattening.
            let nested = match (prefix, include.prefix.as_ref()) {
                (Some(outer), Some(inner)) => Some(format!("{outer}.{}", inner.as_str())),
                (Some(outer), None) => Some(outer.to_string()),
                (None, Some(inner)) => Some(inner.as_str().to_string()),
                (None, None) => None,
            };
            self.absorb(&target, nested.as_deref())
                .with_context(|| format!("included from {}", path.display()))?;
        }
        self.chain.pop();

        let base = self.text.len();
        self.files.push((base, path.clone()));
        self.text.push_str(&source);
        // A newline between files, or the last statement of one and the first
        // of the next would run together in a rendered diagnostic.
        self.text.push('\n');

        expand(&mut diagram, &path)?;
        shift(&mut diagram, base);
        if let Some(prefix) = prefix {
            qualify(&mut diagram, prefix);
        }
        self.merge(diagram, &path)?;
        Ok(())
    }

    fn merge(&mut self, other: Diagram, from: &Path) -> Result<()> {
        for name in declared(&other) {
            if let Some(first) = self.origin.get(&name) {
                bail!(
                    "`{name}` is declared in both {} and {} — give one of the \
                     includes a name with `as`, or rename it",
                    first.display(),
                    from.display()
                );
            }
            self.origin.insert(name, from.to_path_buf());
        }

        let d = &mut self.diagram;
        // The outermost file wins the archetype and the graph settings: an
        // included file describes what it draws, not how the document that
        // pulled it in is laid out. Since the outermost is absorbed *last*,
        // "first one wins" is wrong here and "last one wins" is right.
        if other.archetype.is_some() {
            d.archetype = other.archetype;
        }
        if !other.archetype_props.is_empty() {
            d.archetype_props = other.archetype_props;
        }
        for prop in other.graph {
            replace_prop(&mut d.graph, prop);
        }
        if other.graph_span.is_some() {
            d.graph_span = other.graph_span;
        }
        d.extra_archetypes.extend(other.extra_archetypes);
        d.constraints.extend(other.constraints);
        d.notes.extend(other.notes);
        d.legends.extend(other.legends);
        d.styles.extend(other.styles);
        d.nodes.extend(other.nodes);

        // A group names the edges it spans by *index* into `Diagram::edges` —
        // a sequence diagram's `loop` frame holds messages, not participants.
        // Those indices are relative to the file they were parsed from, so
        // they move by however many edges are already here. Getting this wrong
        // would not fail to compile: a frame would quietly span somebody
        // else's messages.
        let base = d.edges.len();
        let mut groups = other.groups;
        for group in &mut groups {
            for member in &mut group.edge_members {
                *member += base;
            }
        }
        d.groups.extend(groups);
        d.edges.extend(other.edges);
        Ok(())
    }
}

/// Place every `use`, and drop the `component` declarations that fed them.
///
/// A component is an include of a file you did not have to make. So this is
/// the same two steps `include` takes — fill in the text, then prefix what it
/// declares — and it reuses the same [`qualify`] pass, which is why two
/// instances of one component cannot collide.
///
/// Spans inside an instance point at the `use` that placed it rather than at
/// the component's own text: the component is written once and placed many
/// times, so a diagnostic about *this* instance belongs where the instance is.
fn expand(diagram: &mut Diagram, from: &Path) -> Result<()> {
    let components = std::mem::take(&mut diagram.components);
    let uses = std::mem::take(&mut diagram.uses);
    if uses.is_empty() {
        return Ok(());
    }

    for placing in &uses {
        let Some(component) = components
            .iter()
            .find(|c| c.name.as_str() == placing.component.as_str())
        else {
            let known: Vec<&str> = components.iter().map(|c| c.name.as_str()).collect();
            bail!(
                "{}: unknown component `{}`{}",
                from.display(),
                placing.component.as_str(),
                if known.is_empty() {
                    String::new()
                } else {
                    format!(" — this file declares: {}", known.join(", "))
                }
            );
        };

        let mut values = crate::template::Bindings::new();
        for binding in &placing.bindings {
            // Every value written after the key, joined the way the language
            // reads a list — so `size: 120 60` reaches the hole as `120 60`.
            // A single string binds as text, so the engine escapes it for
            // wherever the hole sits — inside a label it is escaped for a
            // string, in a value position it is quoted. Anything else binds
            // as written, joined the way the language reads a list, so
            // `size: 120 60` reaches the hole as `120 60`.
            let value = match binding.values.as_slice() {
                [crate::ast::Value::Str(text)] => crate::template::Value::Text(text.clone()),
                values => crate::template::Value::Raw(
                    values.iter().map(written).collect::<Vec<_>>().join(" "),
                ),
            };
            values.insert(binding.name.clone(), value);
        }

        let filled = crate::template::Template::parse(&component.body)
            .and_then(|t| t.fill(&values))
            .with_context(|| {
                format!(
                    "{}: placing component `{}` as `{}`",
                    from.display(),
                    component.name.as_str(),
                    placing.prefix.as_str()
                )
            })?;

        let mut instance = crate::parser::parse(&filled).map_err(|e| {
            anyhow::anyhow!(
                "{}: component `{}` does not parse once `{}`'s bindings are filled: {}",
                from.display(),
                component.name.as_str(),
                placing.prefix.as_str(),
                e.message
            )
        })?;

        // Every span in the instance points at the `use`. The instance has no
        // text of its own in any file, and a span into the component's body
        // would send a reader to a line that describes every instance rather
        // than the one that is wrong.
        pin(&mut instance, placing.span);
        qualify(&mut instance, placing.prefix.as_str());

        // The instance is wrapped in a frame named after it. That is what
        // makes an instance *placeable*: a group is already anchorable,
        // already sized by what it holds, and already moves its subtree — so
        // `use service as jobs right-of web` needs no placement vocabulary of
        // its own, and `web` names the instance while `web.app` names a box
        // inside it.
        let members: Vec<Spanned<String>> = instance
            .nodes
            .iter()
            .map(|n| n.id.clone())
            .chain(instance.groups.iter().map(|g| g.id.clone()))
            .collect();

        let base = diagram.edges.len();
        let mut groups = instance.groups;
        for group in &mut groups {
            for member in &mut group.edge_members {
                *member += base;
            }
        }
        diagram.groups.extend(groups);
        diagram.edges.extend(instance.edges);
        diagram.nodes.extend(instance.nodes);
        diagram.styles.extend(instance.styles);

        diagram.groups.push(crate::ast::GroupDecl {
            activates: false,
            id: placing.prefix.clone(),
            label: None,
            classes: Vec::new(),
            placement: placing.placement.clone(),
            props: Vec::new(),
            members,
            edge_members: Vec::new(),
            span: placing.span,
        });
    }

    Ok(())
}

/// Point every span in an instance at the `use` that placed it.
///
/// The instance has no text of its own in any file. A span into the
/// component's body would send a reader to the line that describes *every*
/// instance rather than to the one that is wrong.
fn pin(d: &mut Diagram, at: Span) {
    let s = |span: &mut Span| *span = at;
    let sp = |x: &mut Spanned<String>| x.span = at;
    let props = |ps: &mut Vec<Prop>| {
        for p in ps {
            p.span = at;
            p.name_span = at;
            p.values_span = at;
        }
    };

    props(&mut d.graph);
    for style in &mut d.styles {
        sp(&mut style.name);
        if let Some(e) = &mut style.extends {
            sp(e);
        }
        props(&mut style.props);
        s(&mut style.span);
    }
    for node in &mut d.nodes {
        sp(&mut node.keyword);
        sp(&mut node.id);
        s(&mut node.positional_span);
        for c in &mut node.classes {
            sp(c);
        }
        for m in &mut node.members {
            sp(m);
        }
        props(&mut node.props);
        pin_placement(&mut node.placement, at);
        s(&mut node.span);
    }
    for edge in &mut d.edges {
        sp(&mut edge.from);
        sp(&mut edge.to);
        s(&mut edge.positional_span);
        for c in &mut edge.classes {
            sp(c);
        }
        props(&mut edge.props);
        s(&mut edge.span);
    }
    for group in &mut d.groups {
        sp(&mut group.id);
        for c in &mut group.classes {
            sp(c);
        }
        for m in &mut group.members {
            sp(m);
        }
        props(&mut group.props);
        pin_placement(&mut group.placement, at);
        s(&mut group.span);
    }
}

fn pin_placement(placement: &mut Option<crate::ast::Placement>, at: Span) {
    use crate::ast::Placement;
    if let Some(Placement::Rel { anchor, .. }) = placement {
        anchor.span = at;
    }
}

/// A value as it was written, for putting back into source text.
fn written(value: &crate::ast::Value) -> String {
    use crate::ast::Value;
    match value {
        Value::Num(n) => crate::charts::trim_number(*n),
        Value::Str(s) => format!("{s:?}"),
        Value::Word(w) => w.clone(),
        Value::Color(c) => c.clone(),
        other => format!("{other:?}"),
    }
}

/// A later setting replaces an earlier one of the same name rather than
/// stacking beside it, or `background` declared in two files would leave the
/// reader guessing which the renderer took.
fn replace_prop(into: &mut Vec<Prop>, prop: Prop) {
    if let Some(existing) = into.iter_mut().find(|p| p.name == prop.name) {
        *existing = prop;
    } else {
        into.push(prop);
    }
}

/// Every name a file declares — what a clash is detected against.
fn declared(d: &Diagram) -> Vec<String> {
    let mut names = Vec::new();
    names.extend(d.nodes.iter().map(|n| n.id.value.clone()));
    // An activation names an item that already exists rather than declaring
    // one (`D-024`), so counting it would report a clash with itself.
    names.extend(
        d.groups
            .iter()
            .filter(|g| !g.activates)
            .map(|g| g.id.value.clone()),
    );
    names.extend(d.styles.iter().map(|s| format!("style {}", s.name.value)));
    names
}

/// Move every span in a diagram by `delta`.
fn shift(d: &mut Diagram, delta: usize) {
    let s = |span: &mut Span| {
        span.start += delta;
        span.end += delta;
    };
    let sp = |x: &mut Spanned<String>| {
        x.span.start += delta;
        x.span.end += delta;
    };
    let props = |ps: &mut Vec<Prop>| {
        for p in ps {
            p.span.start += delta;
            p.span.end += delta;
            p.name_span.start += delta;
            p.name_span.end += delta;
            p.values_span.start += delta;
            p.values_span.end += delta;
        }
    };

    if let Some(a) = &mut d.archetype {
        sp(a);
    }
    props(&mut d.archetype_props);
    for a in &mut d.extra_archetypes {
        sp(a);
    }
    props(&mut d.graph);
    if let Some(span) = &mut d.graph_span {
        s(span);
    }
    for style in &mut d.styles {
        sp(&mut style.name);
        if let Some(e) = &mut style.extends {
            sp(e);
        }
        props(&mut style.props);
        s(&mut style.span);
    }
    for node in &mut d.nodes {
        sp(&mut node.keyword);
        sp(&mut node.id);
        s(&mut node.positional_span);
        for c in &mut node.classes {
            sp(c);
        }
        for m in &mut node.members {
            sp(m);
        }
        props(&mut node.props);
        shift_placement(&mut node.placement, delta);
        s(&mut node.span);
    }
    for edge in &mut d.edges {
        sp(&mut edge.from);
        sp(&mut edge.to);
        s(&mut edge.positional_span);
        for c in &mut edge.classes {
            sp(c);
        }
        props(&mut edge.props);
        s(&mut edge.span);
    }
    for group in &mut d.groups {
        sp(&mut group.id);
        for c in &mut group.classes {
            sp(c);
        }
        for m in &mut group.members {
            sp(m);
        }
        props(&mut group.props);
        shift_placement(&mut group.placement, delta);
        s(&mut group.span);
    }
}

fn shift_placement(placement: &mut Option<crate::ast::Placement>, delta: usize) {
    use crate::ast::Placement;
    if let Some(Placement::Rel { anchor, .. }) = placement {
        anchor.span.start += delta;
        anchor.span.end += delta;
    }
}

/// Put every name a file declares — and every reference it makes to one of
/// them — under `prefix`.
///
/// A reference the file did *not* declare is left alone: it is either a
/// forward reference into the file that included this one, or a name from a
/// third file, and renaming it would break both.
fn qualify(d: &mut Diagram, prefix: &str) {
    let own: std::collections::HashSet<String> = d
        .nodes
        .iter()
        .map(|n| n.id.value.clone())
        .chain(d.groups.iter().map(|g| g.id.value.clone()))
        .collect();
    let styles: std::collections::HashSet<String> =
        d.styles.iter().map(|s| s.name.value.clone()).collect();

    let rename = |x: &mut Spanned<String>| *x = qualified(x, prefix);

    for node in &mut d.nodes {
        rename(&mut node.id);
        for class in &mut node.classes {
            if styles.contains(&class.value) {
                rename(class);
            }
        }
        rename_anchor(&mut node.placement, &own, prefix);
    }
    for group in &mut d.groups {
        rename(&mut group.id);
        for member in &mut group.members {
            if own.contains(&member.value) {
                rename(member);
            }
        }
        for class in &mut group.classes {
            if styles.contains(&class.value) {
                rename(class);
            }
        }
        rename_anchor(&mut group.placement, &own, prefix);
    }
    for edge in &mut d.edges {
        for end in [&mut edge.from, &mut edge.to] {
            if own.contains(&end.value) {
                *end = qualified(end, prefix);
            }
        }
        for class in &mut edge.classes {
            if styles.contains(&class.value) {
                rename(class);
            }
        }
    }
    for style in &mut d.styles {
        rename(&mut style.name);
        if let Some(parent) = &mut style.extends
            && styles.contains(&parent.value)
        {
            rename(parent);
        }
    }
}

fn rename_anchor(
    placement: &mut Option<crate::ast::Placement>,
    own: &std::collections::HashSet<String>,
    prefix: &str,
) {
    use crate::ast::Placement;
    if let Some(Placement::Rel { anchor, .. }) = placement
        && own.contains(&anchor.value)
    {
        *anchor = qualified(anchor, prefix);
    }
}

fn qualified(name: &Spanned<String>, prefix: &str) -> Spanned<String> {
    Spanned {
        value: format!("{prefix}.{}", name.value),
        span: name.span,
    }
}
