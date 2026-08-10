use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use clap::{Parser as ClapParser, Subcommand};

use bgraph::{layout, parser, render_svg, watch};

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

        /// Also open a window on whichever file changed last. Off by default:
        /// a window that changes subject on its own is one you stop following
        #[arg(long)]
        follow: bool,

        /// Serve the preview over HTTP instead of opening a window — for a
        /// remote machine, a second screen, or a build with no display
        #[arg(long)]
        server: bool,

        /// Bind a template's holes from a JSON object, re-read on every
        /// request — so editing the data reloads the preview too
        #[arg(long = "data", value_name = "FILE")]
        data: Option<PathBuf>,

        #[arg(short, long, default_value_t = 7878)]
        port: u16,
    },
    /// Write the generated reference manual
    Docs {
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
    },
    /// Reformat a file to the canonical shape
    Fmt {
        /// Input DSL files
        inputs: Vec<PathBuf>,

        /// Report which files would change, and change nothing
        #[arg(long)]
        check: bool,
    },
    /// Write a shell completion script, or the man page, to standard output
    #[cfg(feature = "completions")]
    Completions {
        /// bash | elvish | fish | powershell | zsh | man
        shell: String,
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
/// The build loop is the whole of it without `--follow`, and that is the
/// half worth having: regenerating a directory after editing one file is what
/// a repository of `.bd` files beside their committed `.svg` needs.
///
/// `--follow` adds the window, on whichever file changed last. It is opt-in
/// because a window that changes subject on its own is a window you stop
/// trusting — but while editing six diagrams in turn it is exactly right, and
/// the alternative is six windows.
fn watch_dir(
    dir: &std::path::Path,
    out: Option<&std::path::Path>,
    once: bool,
    follow: bool,
) -> Result<i32> {
    if follow && once {
        bail!("`--once` builds and stops, so there is nothing for `--follow` to follow");
    }

    #[cfg(feature = "window")]
    if follow {
        if !bgraph::window::available() {
            bail!(
                "`--follow` needs a display; without one, build with no flag and open what you want"
            );
        }
        // The loop runs beside the window rather than inside it: the pictures
        // must keep being written whether or not anyone is looking at one.
        let building = dir.to_path_buf();
        let target = out.map(|p| p.to_path_buf());
        std::thread::spawn(move || {
            if let Err(err) = watch::build_dir(&building, target.as_deref(), false) {
                eprintln!("bgraph: the build loop stopped: {err:#}");
            }
        });
        bgraph::window::follow(dir.to_path_buf())?;
        return Ok(0);
    }
    #[cfg(not(feature = "window"))]
    if follow {
        bail!("this build has no window — rebuild with `--features window`");
    }

    watch::build_dir(dir, out, once)?;
    Ok(0)
}

/// Two versions of a diagram, drawn as one.
///
/// Matched by identifier, which the language already gives every box — so
/// this is a set difference on the solved geometry rather than an image
/// comparison, and it stays vector and deterministic.
fn diff_files(before: &std::path::Path, after: &std::path::Path, output: &PathBuf) -> Result<i32> {
    let old = layout::solve(&bgraph::compose::load(before)?.diagram)
        .with_context(|| format!("in {}", before.display()))?;
    let new = layout::solve(&bgraph::compose::load(after)?.diagram)
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
/// A window when there is one to open, the HTTP preview otherwise.
///
/// The window is the default because it is what "show me this" means on a
/// desktop. The server is not an apology — it is what a remote machine and a
/// second screen both want, and `--server` asks for it outright.
fn show_or_serve(input: PathBuf, data: Option<PathBuf>, port: u16, server: bool) -> Result<i32> {
    #[cfg(feature = "window")]
    if !server {
        if bgraph::window::available() {
            bgraph::window::show(input, data)?;
            return Ok(0);
        }
        eprintln!(
            "bgraph: no display, so {} is served instead of shown",
            bgraph::window::describe(&input)
        );
    }
    #[cfg(not(feature = "window"))]
    if !server {
        eprintln!("bgraph: this build has no window — rebuild with `--features window`");
    }

    watch::serve(input, data, port)?;
    Ok(0)
}

fn format_files(inputs: &[PathBuf], check: bool) -> Result<i32> {
    if inputs.is_empty() {
        bail!("no input file (try `bgraph fmt diagram.bd`)");
    }

    let mut changed = 0;
    for path in inputs {
        let source =
            fs::read_to_string(path).with_context(|| format!("cannot read {}", path.display()))?;
        let formatted = bgraph::fmt::format(&source)
            .with_context(|| format!("{} does not parse, so it is left alone", path.display()))?;

        if formatted == source {
            continue;
        }
        changed += 1;
        if check {
            println!("{}", path.display());
        } else {
            fs::write(path, &formatted)
                .with_context(|| format!("cannot write {}", path.display()))?;
            println!("{} reformatted", path.display());
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
            follow,
            data,
            port,
            server,
        }) => {
            if input.is_dir() {
                return watch_dir(&input, out.as_deref(), once, follow);
            }
            if once || out.is_some() || follow {
                bail!(
                    "`--out`, `--once` and `--follow` are for a directory; \
                     for one file use `-o` and `bgraph diagram.bd`"
                );
            }
            return show_or_serve(input, data, port, server);
        }
        Some(Command::Check { input, ci }) => return check_file(&input, ci),
        Some(Command::Fmt { inputs, check }) => return format_files(&inputs, check),
        #[cfg(feature = "completions")]
        Some(Command::Completions { shell }) => return completions(&shell),
        Some(Command::Init { genre, output }) => return init(genre, output),
        Some(Command::Explain { input, name }) => {
            let composed = bgraph::compose::load(&input)?;
            let solved = layout::solve(&composed.diagram)?;
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
        None => {}
    }

    let Some(input) = cli.input else {
        bail!("no input file (try `bgraph diagram.bd -o diagram.svg`)");
    };

    let source =
        fs::read_to_string(&input).with_context(|| format!("cannot read {}", input.display()))?;

    // A file with holes is a template; one without renders exactly as before.
    let template = bgraph::Template::parse(&source)?;
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

    // Through the composer rather than the parser, so `include` resolves
    // against the file that wrote it. A file with no includes takes exactly
    // the same path and comes out the same.
    let mut composed = bgraph::compose::load(&input)?;
    if let Some(theme) = &cli.theme {
        bgraph::themes::apply(&mut composed.diagram, theme)?;
    }
    let layout = layout::solve(&composed.diagram)?;

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

/// The same report, for a diagram spread over several files.
///
/// Recovery is not attempted here: it works by cutting one text into
/// statements, and there is no one text once files are merged. A composed
/// diagram that parses at all is checked in full, which is the case that
/// matters — a syntax error in an included file was already reported by name
/// when the composer refused to load it.
fn report_composed(
    input: &std::path::Path,
    composed: &bgraph::compose::Composed,
    ci: bool,
) -> Result<i32> {
    use bgraph::check::Severity;
    use bgraph::report;

    let mut diagnostics = bgraph::check::check(&composed.diagram);
    if let Ok(solved) = layout::solve(&composed.diagram) {
        diagnostics.extend(bgraph::check::after_layout(&solved));
    }
    diagnostics.sort_by_key(|d| (d.span.start, d.span.end));

    let (rich, colour) = report::decorate(ci);
    let mut errors = 0;

    for diag in &diagnostics {
        // Through the map, so a problem in an included file is reported
        // against *that* file at *its* line — the whole reason the composer
        // keeps one.
        let (path, line, col) = composed.map.locate(diag.span.start);
        let path = path.display().to_string();
        if diag.severity == Severity::Error {
            errors += 1;
        }
        if rich {
            let text = composed.map.source_of(diag.span.start);
            println!("{}\n", report::rich(&path, text, line, col, diag, colour));
        } else {
            println!("{}", report::flat(&path, line, col, diag));
        }
    }

    if diagnostics.is_empty() {
        println!("{}: no problems found", input.display());
    }
    Ok(i32::from(errors > 0))
}

/// Prints every diagnostic in `file:line:col: severity: message` form so it
/// drops straight into an editor's error list.
fn check_file(input: &PathBuf, ci: bool) -> Result<i32> {
    use bgraph::check::Severity;
    use bgraph::lsp::LineIndex;
    use bgraph::report;

    let source =
        fs::read_to_string(input).with_context(|| format!("cannot read {}", input.display()))?;
    let index = LineIndex::new(&source);

    // Composed first, so a diagram spread over several files is checked as
    // the one diagram it is. When that fails — a circular include, a name in
    // two files — there is nothing to check yet, so the failure is the report.
    match bgraph::compose::load(input) {
        Ok(composed) => {
            if !composed.diagram.includes.is_empty() || composed.map.text != source {
                return report_composed(input, &composed, ci);
            }
        }
        Err(err) => {
            eprintln!("error: {err:#}");
            return Ok(1);
        }
    }

    // Every mistake, not the first: a person fixing a file wants the list,
    // and a build log that stops at line 3 hides what is on line 40.
    let (diagram, syntax) = parser::parse_recovering(&source);

    let mut diagnostics: Vec<bgraph::check::Diag> = syntax
        .into_iter()
        .map(|err| bgraph::check::Diag {
            severity: Severity::Error,
            message: err.message,
            span: err.span,
        })
        .collect();

    if let Some(diagram) = &diagram {
        diagnostics.extend(bgraph::check::check(diagram));
        // The ones that need the boxes placed. `check` is still the only
        // validator; this is its second pass, not a second validator.
        if let Ok(solved) = layout::solve(diagram) {
            diagnostics.extend(bgraph::check::after_layout(&solved));
        }
    }
    diagnostics.sort_by_key(|d| (d.span.start, d.span.end));

    let (rich, colour) = report::decorate(ci);
    let path = input.display().to_string();

    let mut errors = 0;
    for diag in &diagnostics {
        let (line, col) = index.line_col(diag.span.start);
        if diag.severity == Severity::Error {
            errors += 1;
        }
        if rich {
            // A blank line between them, or two diagnostics from adjacent
            // lines read as one five-line block.
            println!(
                "{}\n",
                report::rich(&path, &source, line, col, diag, colour)
            );
        } else {
            println!("{}", report::flat(&path, line, col, diag));
        }
    }

    if diagnostics.is_empty() {
        println!("{}: no problems found", input.display());
    }
    Ok(i32::from(errors > 0))
}
