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
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Show a diagram and reload it when it changes. Takes a `.bd` to render
    /// or an `.svg` to show as it is — the extension decides
    Watch {
        /// Input `.bd` or `.svg` file
        input: PathBuf,

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
            data,
            port,
            server,
        }) => return show_or_serve(input, data, port, server),
        Some(Command::Check { input }) => return check_file(&input),
        Some(Command::Fmt { inputs, check }) => return format_files(&inputs, check),
        #[cfg(feature = "completions")]
        Some(Command::Completions { shell }) => return completions(&shell),
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

    let diagram = parser::parse_diagram(&source)?;
    let layout = layout::solve(&diagram)?;

    // The output extension picks the writer. A flag would be a second way to
    // say what the filename already says, and the two could disagree.
    let wants_pdf = cli
        .output
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("pdf"));

    if wants_pdf {
        #[cfg(feature = "pdf")]
        {
            fs::write(&cli.output, bgraph::render_pdf::render(&layout))
                .with_context(|| format!("cannot write {}", cli.output.display()))?;
        }
        #[cfg(not(feature = "pdf"))]
        bail!("this build has no PDF writer — rebuild with the `pdf` feature");
    } else {
        fs::write(&cli.output, render_svg::render(&layout))
            .with_context(|| format!("cannot write {}", cli.output.display()))?;
    }

    println!(
        "{} nodes, {} edges → {}",
        layout.nodes.len(),
        layout.edges.len(),
        cli.output.display()
    );
    Ok(0)
}

/// Prints every diagnostic in `file:line:col: severity: message` form so it
/// drops straight into an editor's error list.
fn check_file(input: &PathBuf) -> Result<i32> {
    use bgraph::check::Severity;
    use bgraph::lsp::LineIndex;

    let source =
        fs::read_to_string(input).with_context(|| format!("cannot read {}", input.display()))?;
    let index = LineIndex::new(&source);

    let mut diagnostics = match parser::parse(&source) {
        Ok(diagram) => bgraph::check::check(&diagram),
        Err(err) => vec![bgraph::check::Diag {
            severity: Severity::Error,
            message: err.message,
            span: err.span,
        }],
    };

    // The ones that need the boxes placed. `check` is still the only
    // validator; this is its second pass, not a second one.
    if let Ok(diagram) = parser::parse(&source)
        && let Ok(solved) = layout::solve(&diagram)
    {
        diagnostics.extend(bgraph::check::after_layout(&solved));
    }
    diagnostics.sort_by_key(|d| (d.span.start, d.span.end));

    let mut errors = 0;
    for diag in &diagnostics {
        let (line, col) = index.line_col(diag.span.start);
        let label = match diag.severity {
            Severity::Error => {
                errors += 1;
                "error"
            }
            Severity::Warning => "warning",
        };
        println!(
            "{}:{}:{}: {label}: {}",
            input.display(),
            line + 1,
            col + 1,
            diag.message
        );
    }

    if diagnostics.is_empty() {
        println!("{}: no problems found", input.display());
    }
    Ok(i32::from(errors > 0))
}
