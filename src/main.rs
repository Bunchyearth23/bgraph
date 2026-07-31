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
        None => {}
    }

    let Some(input) = cli.input else {
        bail!("no input file (try `bgraph diagram.bd -o diagram.svg`)");
    };

    let source =
        fs::read_to_string(&input).with_context(|| format!("cannot read {}", input.display()))?;
    let diagram = parser::parse_diagram(&source)?;
    let layout = layout::solve(&diagram)?;
    let svg = render_svg::render(&layout);

    fs::write(&cli.output, svg)
        .with_context(|| format!("cannot write {}", cli.output.display()))?;

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
