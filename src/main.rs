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

    /// Print the filled source instead of rendering it
    #[arg(long)]
    emit: bool,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Serve a live-reloading preview of the diagram
    Watch {
        /// Input DSL file
        input: PathBuf,

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
        Some(Command::Watch { input, port }) => {
            watch::serve(input, port)?;
            return Ok(0);
        }
        Some(Command::Check { input }) => return check_file(&input),
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
    if !template.holes().is_empty() || !cli.set.is_empty() {
        let mut values = bgraph::Bindings::new();
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

    let diagnostics = match parser::parse(&source) {
        Ok(diagram) => bgraph::check::check(&diagram),
        Err(err) => vec![bgraph::check::Diag {
            severity: Severity::Error,
            message: err.message,
            span: err.span,
        }],
    };

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
