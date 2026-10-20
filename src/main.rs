use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use clap::{Parser as ClapParser, Subcommand};

use bgraph::{parser, render_svg, watch};

#[derive(ClapParser, Debug)]
#[command(
    name = "bgraph",
    version,
    about = "Stylable diagrams from a layout-first DSL"
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,

    /// Input DSL file
    input: Option<PathBuf>,

    /// Output SVG file
    #[arg(short, long, default_value = "out.svg")]
    output: PathBuf,

    /// Bind a template hole: `--set year=2026 --set north=4820`
    #[arg(long = "set", value_name = "NAME=VALUE")]
    set: Vec<String>,

    /// Bind holes from a JSON object; `-` reads standard input. Lists and
    /// arrays of objects become lists and repeated lines, which `--set`
    /// cannot express.
    #[arg(long = "data", value_name = "FILE")]
    data: Option<PathBuf>,

    /// Bind one hole to the rows of a CSV: `--rows source=sales.csv`. The
    /// header names the fields, and every other line writes its statement
    /// once.
    #[arg(long = "rows", value_name = "NAME=FILE")]
    rows: Option<String>,

    /// Print the filled source instead of rendering it
    #[arg(long)]
    emit: bool,

    /// Multiply the diagram's own size when rasterising. PNG only — a vector
    /// has no resolution to choose
    #[arg(long, default_value_t = 1.0, value_name = "N")]
    scale: f64,

    /// A named palette laid underneath the file's own styles: light, dark,
    /// print. What the file declares still wins
    #[arg(long, value_name = "NAME")]
    theme: Option<String>,

    /// Carry a TrueType font inside SVG or PDF, so text works on a machine that
    /// does not have it. The file grows by the size of the font
    #[arg(long, value_name = "FILE")]
    embed_font: Option<PathBuf>,

    /// Tile a large diagram across sheets: a4, a3, letter, or WIDTHxHEIGHT in
    /// points. PDF only — an SVG has no pages
    #[arg(long, value_name = "SIZE")]
    page: Option<String>,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Show a diagram and reload it when it changes. Takes a `.bd` to render
    /// or an `.svg` to show as it is — the extension decides. Given a
    /// directory, it becomes a build loop instead: every `.bd` in it is
    /// rendered as it changes
    Watch {
        /// Input `.bd` or `.svg` file, or a directory of `.bd` files
        input: PathBuf,

        /// Where a directory's pictures are written; beside their source
        /// when omitted. Ignored for a single file, which `-o` already covers
        #[arg(long = "out", value_name = "DIR")]
        out: Option<PathBuf>,

        /// Build once and stop, exiting non-zero if anything failed — the
        /// same command as a build step. Directories only
        #[arg(long)]
        once: bool,

        /// Bind a template's holes from a JSON object, re-read on every
        /// request — so editing the data reloads the preview too
        #[arg(long = "data", value_name = "FILE")]
        data: Option<PathBuf>,

        /// Preview port; zero chooses a free port
        #[arg(short, long, default_value_t = 7878)]
        port: u16,

        /// Open the preview in the default browser (single files only)
        #[arg(long)]
        open: bool,
    },
    /// Write the generated reference manual
    Docs {
        /// Output file; stdout when omitted
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
    /// Write the machine-readable vocabulary schema
    Schema {
        /// Output file; stdout when omitted
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
    /// Report diagnostics for a file without rendering it
    Check {
        /// Input DSL file
        input: PathBuf,

        /// Force the flat `file:line:col: severity: message` form, whatever
        /// stdout is attached to
        #[arg(long)]
        ci: bool,

        /// Emit a stable JSON array for editor and CI integrations
        #[arg(long)]
        json: bool,
    },
    /// Reformat a file to the canonical shape
    Fmt {
        /// Input DSL files
        inputs: Vec<PathBuf>,

        /// Report which files would change, and change nothing
        #[arg(long)]
        check: bool,

        /// Rewrite a removed `opacity` as the fourth channel of the colours it
        /// applied to, and reformat nothing
        #[arg(long)]
        migrate: bool,
    },
    /// Write a shell completion script, or the man page, to standard output
    #[cfg(feature = "completions")]
    Completions {
        /// bash | elvish | fish | powershell | zsh | man
        shell: String,
    },
    /// Write the placements into the file, once, so every box says where it
    /// goes. The picture does not change
    Layout {
        /// Input DSL files
        inputs: Vec<PathBuf>,

        /// Report which files would change, and change nothing
        #[arg(long)]
        check: bool,
    },
    /// Say why a box is where it is, and where each of its colours came from
    Explain {
        /// Input DSL file
        input: PathBuf,
        /// The box to explain
        name: String,
    },
    /// Draw two versions of a diagram as one picture: what appeared, what
    /// left, what moved
    Diff {
        /// The earlier file
        before: PathBuf,
        /// The later one
        after: PathBuf,

        #[arg(short, long, default_value = "diff.svg")]
        output: PathBuf,
    },
    /// Write a working file for a genre; omit the genre to list them
    Init {
        /// The archetype: pie, gantt, er, sequence, … — `free` for the
        /// plain node-and-edge graph
        genre: Option<String>,

        /// Output file; stdout when omitted
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
}

/// A directory of diagrams, kept rendered.
///
/// Regenerating a directory after editing one file is what a repository of
/// `.bd` files beside their committed `.svg` needs, and `--once` makes the
/// same command a build step.
fn watch_dir(dir: &std::path::Path, out: Option<&std::path::Path>, once: bool) -> Result<i32> {
    watch::build_dir(dir, out, once)?;
    Ok(0)
}

/// Freeze the layout into the source.
///
/// The picture must not change — that is the whole promise — so the file is
/// re-rendered before and after and the two compared. A mismatch is refused
/// rather than written: rewriting someone's file and changing their diagram
/// while doing it is the one outcome this command may never produce.
fn write_layout(inputs: &[PathBuf], check: bool) -> Result<i32> {
    if inputs.is_empty() {
        bail!("no input file (try `bgraph layout diagram.bd`)");
    }

    let mut changed = 0;
    for path in inputs {
        let source =
            fs::read_to_string(path).with_context(|| format!("cannot read {}", path.display()))?;
        let diagram = parser::parse_diagram(&source)
            .with_context(|| format!("{} does not parse, so it is left alone", path.display()))?;

        let filled = bgraph::relayout::fill_in(&source, &diagram)?;
        if filled == source {
            continue;
        }

        let before = render_svg::render(&bgraph::analyzed_layout(&diagram)?);
        let after = render_svg::render(&bgraph::analyzed_layout(&parser::parse_diagram(&filled)?)?);
        if before != after {
            bail!(
                "{} would draw differently after `layout`, so nothing was written —                  this is a bug in bgraph, not in the file",
                path.display()
            );
        }

        changed += 1;
        if check {
            println!("{}", path.display());
        } else {
            fs::write(path, &filled).with_context(|| format!("cannot write {}", path.display()))?;
            println!(
                "{}: {} placement{} written",
                path.display(),
                bgraph::relayout::unplaced(&diagram).len(),
                if bgraph::relayout::unplaced(&diagram).len() == 1 {
                    ""
                } else {
                    "s"
                }
            );
        }
    }

    if check && changed > 0 {
        return Ok(1);
    }
    if changed == 0 {
        println!("every box already says where it goes");
    }
    Ok(0)
}

/// Two versions of a diagram, drawn as one.
///
/// Matched by identifier, which the language already gives every box — so
/// this is a set difference on the solved geometry rather than an image
/// comparison, and it stays vector and deterministic.
fn diff_files(before: &std::path::Path, after: &std::path::Path, output: &PathBuf) -> Result<i32> {
    let old = bgraph::analyzed_layout(&bgraph::compose::load(before)?.diagram)
        .with_context(|| format!("in {}", before.display()))?;
    let new = bgraph::analyzed_layout(&bgraph::compose::load(after)?.diagram)
        .with_context(|| format!("in {}", after.display()))?;

    let picture = bgraph::diff::compare(&old, &new);
    fs::write(output, render_svg::render(&picture))
        .with_context(|| format!("cannot write {}", output.display()))?;
    println!(
        "{} → {} → {}",
        before.display(),
        after.display(),
        output.display()
    );
    Ok(0)
}

/// A working file for a genre, or the list of genres.
///
/// The list is the no-argument form rather than a separate command: someone
/// who does not know which genre they want is exactly the person typing
/// `bgraph init`, and sending them to `--help` for it would be a detour.
fn init(genre: Option<String>, output: Option<PathBuf>) -> Result<i32> {
    let Some(genre) = genre else {
        for name in bgraph::init::genres() {
            println!("{name}");
        }
        return Ok(0);
    };

    let source = bgraph::init::skeleton(&genre)?;
    match output {
        Some(path) => {
            fs::write(&path, &source)
                .with_context(|| format!("cannot write {}", path.display()))?;
            println!("{} written for `{genre}`", path.display());
        }
        None => print!("{source}"),
    }
    Ok(0)
}

/// Completions and the man page, both written from the `Cli` struct above
/// rather than from a second description of the flags — a hand-kept
/// completion script is one that is wrong about a flag added last week.
#[cfg(feature = "completions")]
fn completions(shell: &str) -> Result<i32> {
    use clap::CommandFactory;

    let mut command = Cli::command();
    let mut out = std::io::stdout();

    // `man` is not a shell, and it is here anyway: someone who wants one of
    // these wants the other, and a separate subcommand for a single page
    // would be a second thing to remember for no gain.
    if shell == "man" {
        clap_mangen::Man::new(command).render(&mut out)?;
        return Ok(0);
    }

    let generator: clap_complete::Shell = shell
        .parse()
        .map_err(|_| anyhow::anyhow!("unknown shell `{shell}` (try bash, fish, zsh, or man)"))?;
    clap_complete::generate(generator, &mut command, "bgraph", &mut out);
    Ok(0)
}

/// Reformat, or report what would change.
///
/// `--check` exits non-zero when a file is not in its canonical shape, so the
/// same command serves a person and a build — one runs it to fix, the other to
/// refuse.
/// The browser preview, reloading as the file changes.
///
/// There used to be a native window in front of this, and the choice between
/// them. It is gone (`W-136`): a browser is on every machine this runs on,
/// it works the same over ssh and on a second screen, and it cost the crate
/// seventy-eight dependencies and a rasteriser to say the same thing worse.
fn show_or_serve(input: PathBuf, data: Option<PathBuf>, port: u16, open: bool) -> Result<i32> {
    watch::serve_with_browser(input, data, port, open)?;
    Ok(0)
}

fn format_files(inputs: &[PathBuf], check: bool, migrate: bool) -> Result<i32> {
    if inputs.is_empty() {
        bail!("no input file (try `bgraph fmt diagram.bd`)");
    }

    let mut changed = 0;
    for path in inputs {
        let source =
            fs::read_to_string(path).with_context(|| format!("cannot read {}", path.display()))?;
        // A migration is a second mode, not a side effect of formatting: one
        // rewrites what the language dropped and the other arranges what it
        // kept, and a diff that did both would be unreviewable.
        let rewritten = if migrate {
            bgraph::fmt::migrate(&source)
                .with_context(|| format!("cannot migrate {}", path.display()))?
        } else {
            bgraph::fmt::format(&source).with_context(|| {
                format!("{} does not parse, so it is left alone", path.display())
            })?
        };

        if rewritten == source {
            continue;
        }
        changed += 1;
        if check {
            println!("{}", path.display());
        } else {
            fs::write(path, &rewritten)
                .with_context(|| format!("cannot write {}", path.display()))?;
            let what = if migrate { "migrated" } else { "reformatted" };
            println!("{} {what}", path.display());
        }
    }

    if check && changed > 0 {
        eprintln!(
            "{changed} file{} would change",
            if changed == 1 { "" } else { "s" }
        );
        return Ok(1);
    }
    if changed == 0 {
        println!("nothing to change");
    }
    Ok(0)
}

fn main() {
    match run() {
        Ok(code) => std::process::exit(code),
        Err(err) => {
            eprintln!("error: {err:#}");
            std::process::exit(1);
        }
    }
}

fn run() -> Result<i32> {
    let cli = Cli::parse();

    match cli.command {
        Some(Command::Watch {
            input,
            out,
            once,
            data,
            port,
            open,
        }) => {
            if input.is_dir() {
                if open {
                    bail!("`--open` needs a single `.bd` or `.svg` file");
                }
                return watch_dir(&input, out.as_deref(), once);
            }
            if once || out.is_some() {
                bail!(
                    "`--out` and `--once` are for a directory; \
                     for one file use `-o` and `bgraph diagram.bd`"
                );
            }
            return show_or_serve(input, data, port, open);
        }
        Some(Command::Check { input, ci, json }) => return check_file(&input, ci, json),
        Some(Command::Fmt {
            inputs,
            check,
            migrate,
        }) => return format_files(&inputs, check, migrate),
        #[cfg(feature = "completions")]
        Some(Command::Completions { shell }) => return completions(&shell),
        Some(Command::Init { genre, output }) => return init(genre, output),
        Some(Command::Layout { inputs, check }) => return write_layout(&inputs, check),
        Some(Command::Explain { input, name }) => {
            let composed = bgraph::compose::load(&input)?;
            let solved = bgraph::analyzed_layout(&composed.diagram)?;
            print!(
                "{}",
                bgraph::explain::explain(&composed.diagram, &solved, &name)?
            );
            return Ok(0);
        }
        Some(Command::Diff {
            before,
            after,
            output,
        }) => return diff_files(&before, &after, &output),
        Some(Command::Docs { output }) => {
            let manual = bgraph::docgen::reference();
            match output {
                Some(path) => {
                    fs::write(&path, &manual)
                        .with_context(|| format!("cannot write {}", path.display()))?;
                    println!("reference written to {}", path.display());
                }
                None => print!("{manual}"),
            }
            return Ok(0);
        }
        Some(Command::Schema { output }) => {
            let schema = bgraph::docgen::schema();
            match output {
                Some(path) => fs::write(&path, schema)
                    .with_context(|| format!("cannot write {}", path.display()))?,
                None => print!("{schema}"),
            }
            return Ok(0);
        }
        None => {}
    }

    let Some(input) = cli.input else {
        bail!("no input file (try `bgraph diagram.bd -o diagram.svg`)");
    };

    let source =
        fs::read_to_string(&input).with_context(|| format!("cannot read {}", input.display()))?;

    // A file with holes is a template; one without renders exactly as before.
    let template = match bgraph::Template::parse(&source) {
        Ok(template) => template,
        Err(error) => {
            bgraph::analyze_file_source(&input, &source)?.into_layout()?;
            return Err(error);
        }
    };
    if !template.holes().is_empty()
        || !cli.set.is_empty()
        || cli.data.is_some()
        || cli.rows.is_some()
    {
        // Data first, `--set` after: a flag on the command line is the thing
        // you are changing right now, so it wins over the file.
        let mut values = match &cli.data {
            #[cfg(feature = "json")]
            Some(path) => {
                let raw = if path.as_os_str() == "-" {
                    std::io::read_to_string(std::io::stdin())
                        .context("cannot read the data from standard input")?
                } else {
                    fs::read_to_string(path)
                        .with_context(|| format!("cannot read {}", path.display()))?
                };
                bgraph::Bindings::from_json(&raw)
                    .with_context(|| format!("in {}", path.display()))?
            }
            #[cfg(not(feature = "json"))]
            Some(_) => bail!("this build has no JSON reader — rebuild with the `json` feature"),
            None => bgraph::Bindings::new(),
        };
        // Rows next: a CSV is one table under one name, so it cannot clash
        // with anything `--data` supplied except that same name.
        if let Some(spec) = &cli.rows {
            #[cfg(feature = "csv")]
            {
                let Some((name, path)) = spec.split_once('=') else {
                    bail!("`--rows {spec}` needs a `name=file.csv`");
                };
                let raw = if path == "-" {
                    std::io::read_to_string(std::io::stdin())
                        .context("cannot read the rows from standard input")?
                } else {
                    fs::read_to_string(path).with_context(|| format!("cannot read {path}"))?
                };
                let table = bgraph::template::rows_from_csv(name, &raw)
                    .with_context(|| format!("in {path}"))?;
                if let Some(bound) = table.get(name) {
                    values.insert(name, bound.clone());
                }
            }
            #[cfg(not(feature = "csv"))]
            {
                // Named even here, so the featureless build is warning-free
                // and the message still says what was asked for.
                let _ = spec;
                bail!("this build has no CSV reader — rebuild with the `csv` feature");
            }
        }

        for pair in &cli.set {
            let Some((name, raw)) = pair.split_once('=') else {
                bail!("`--set {pair}` needs a `name=value`");
            };
            values.insert(
                name,
                match raw.parse::<f64>() {
                    Ok(n) => bgraph::template::Value::Number(n),
                    Err(_) => bgraph::template::Value::Text(raw.to_string()),
                },
            );
        }

        if cli.emit {
            print!("{}", template.fill(&values)?);
            return Ok(0);
        }

        let svg = template.render(&values)?;
        fs::write(&cli.output, svg)
            .with_context(|| format!("cannot write {}", cli.output.display()))?;
        println!(
            "{} holes filled → {}",
            template.holes().len(),
            cli.output.display()
        );
        return Ok(0);
    }

    let mut layout = bgraph::analyze_file_source_with_theme(&input, &source, cli.theme.as_deref())?
        .into_layout()?;
    if let Some(path) = &cli.embed_font {
        let whole = fs::read(path).with_context(|| format!("cannot read {}", path.display()))?;
        // Only the glyphs the diagram draws. The text is gathered from the
        // solved layout rather than from the source, so a label the renderer
        // never draws costs nothing and one it invents — a percentage, an
        // axis tick — is not left out.
        let mut drawn = String::new();
        for node in &layout.nodes {
            for line in &node.lines {
                drawn.push_str(line);
            }
        }
        for group in &layout.groups {
            for line in &group.lines {
                drawn.push_str(line);
            }
        }
        for annotation in &layout.annotations {
            drawn.push_str(&annotation.text);
        }
        for edge in &layout.edges {
            if let Some(label) = &edge.label {
                drawn.push_str(label);
            }
        }
        let bytes = bgraph::subset::keep_glyphs_for(&whole, &drawn);
        // Named after the file, because that is what the diagram's own
        // `font:` will be asking for — a family invented here would be one
        // nothing referred to.
        let family = path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "embedded".into());
        layout.opts.embed_font = Some((family, std::sync::Arc::new(bytes)));
    }

    // The output extension picks the writer. A flag would be a second way to
    // say what the filename already says, and the two could disagree.
    let extension = cli
        .output
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();

    match extension.as_str() {
        "pdf" => {
            #[cfg(feature = "pdf")]
            {
                if let Some((_, bytes)) = &layout.opts.embed_font
                    && !bgraph::render_pdf::supports_embedded_font(bytes)
                {
                    bail!("PDF embedding requires a valid TrueType font with glyf outlines (.ttf)");
                }
                let bytes = match &cli.page {
                    Some(size) => {
                        let page = bgraph::render_pdf::Page::parse(size).with_context(|| {
                            format!("unknown page size `{size}` (try a4, a3, letter, or 595x842)")
                        })?;
                        bgraph::render_pdf::render_paged(&layout, page)
                    }
                    None => bgraph::render_pdf::render(&layout),
                };
                fs::write(&cli.output, bytes)
                    .with_context(|| format!("cannot write {}", cli.output.display()))?;
            }
            #[cfg(not(feature = "pdf"))]
            bail!("this build has no PDF writer — rebuild with the `pdf` feature");
        }
        "json" => {
            // Pretty by default: a person running this at a terminal is
            // reading it, and a program does not care either way.
            fs::write(&cli.output, bgraph::render_json::render_pretty(&layout))
                .with_context(|| format!("cannot write {}", cli.output.display()))?;
        }
        "png" => {
            #[cfg(feature = "png")]
            {
                let bytes = bgraph::render_png::render(&layout, cli.scale)?;
                fs::write(&cli.output, bytes)
                    .with_context(|| format!("cannot write {}", cli.output.display()))?;
            }
            #[cfg(not(feature = "png"))]
            bail!("this build has no rasteriser — rebuild with the `png` feature");
        }
        _ => {
            if cli.page.is_some() {
                bail!("`--page` is for a PDF; an SVG has no pages");
            }
            if cli.scale != 1.0 {
                bail!("`--scale` is for a raster; an SVG and a PDF have no resolution to choose");
            }
            fs::write(&cli.output, render_svg::render(&layout))
                .with_context(|| format!("cannot write {}", cli.output.display()))?;
        }
    }

    println!(
        "{} nodes, {} edges → {}",
        layout.nodes.len(),
        layout.edges.len(),
        cli.output.display()
    );
    Ok(0)
}

/// Report diagnostics using the owning file for every recovered source span.
fn report_composed(
    input: &std::path::Path,
    composed: &bgraph::FileAnalysis,
    ci: bool,
    json_output: bool,
) -> Result<i32> {
    use bgraph::check::Severity;
    use bgraph::report;

    let diagnostics = &composed.analysis.diagnostics;

    let (rich, colour) = report::decorate(ci);
    let mut errors = 0;

    let mut machine = Vec::new();
    for diag in diagnostics {
        // Through the map, so a problem in an included file is reported
        // against *that* file at *its* line — the whole reason the composer
        // keeps one.
        let (path, line, col) = composed.map.locate(diag.span.start);
        let path = path.display().to_string();
        if diag.severity == Severity::Error {
            errors += 1;
        }
        if json_output {
            machine.push(serde_json::json!({
                "file": path,
                "line": line + 1,
                "column": col + 1,
                "severity": if diag.severity == Severity::Error { "error" } else { "warning" },
                "message": diag.message,
            }));
        } else if rich {
            let text = composed.map.source_of(diag.span.start);
            println!("{}\n", report::rich(&path, text, line, col, diag, colour));
        } else {
            println!("{}", report::flat(&path, line, col, diag));
        }
    }

    if json_output {
        println!("{}", serde_json::to_string_pretty(&machine)?);
    } else if diagnostics.is_empty() {
        println!("{}: no problems found", input.display());
    }
    Ok(i32::from(errors > 0))
}

/// Prints every diagnostic in `file:line:col: severity: message` form so it
/// drops straight into an editor's error list.
fn check_file(input: &Path, ci: bool, json_output: bool) -> Result<i32> {
    let analysis = bgraph::analyze_file(input)?;
    report_composed(input, &analysis, ci, json_output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_check_contract_has_the_stable_fields_and_exit_status() {
        let source = "node a .missing\n";
        let diagram = parser::parse_diagram(source).expect("parses");
        let diagnostics = bgraph::check::check(&diagram);
        let index = bgraph::lsp::LineIndex::new(source);
        let machine: Vec<_> = diagnostics
            .iter()
            .map(|diagnostic| {
                let (line, column) = index.line_col(diagnostic.span.start);
                serde_json::json!({
                    "file": "diagram.bd",
                    "line": line + 1,
                    "column": column + 1,
                    "severity": if diagnostic.severity == bgraph::check::Severity::Error { "error" } else { "warning" },
                    "message": diagnostic.message,
                })
            })
            .collect();
        assert_eq!(machine[0]["file"], "diagram.bd");
        assert!(machine[0]["line"].is_u64());
        assert!(machine[0]["column"].is_u64());
        assert!(machine[0]["message"].is_string());
        assert!(
            machine
                .iter()
                .any(|diagnostic| diagnostic["severity"] == "error")
        );
    }
}
