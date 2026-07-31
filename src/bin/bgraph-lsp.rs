//! Language server for the bgraph DSL. Speaks LSP over stdin/stdout; the Zed
//! extension (and any other LSP client) launches it with no arguments.

fn main() {
    if let Err(err) = bgraph::lsp::run() {
        eprintln!("bgraph-lsp: {err}");
        std::process::exit(1);
    }
}
