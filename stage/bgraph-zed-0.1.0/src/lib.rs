//! Zed extension for the bgraph DSL.
//!
//! Highlighting comes from the tree-sitter grammar declared in
//! `extension.toml`; everything semantic comes from `bgraph-lsp`, which this
//! extension only has to locate and launch.

use zed_extension_api::{self as zed, Command, LanguageServerId, Result, Worktree};

struct BgraphExtension;

const SERVER: &str = "bgraph-lsp";

impl zed::Extension for BgraphExtension {
    fn new() -> Self {
        BgraphExtension
    }

    fn language_server_command(
        &mut self,
        language_server_id: &LanguageServerId,
        worktree: &Worktree,
    ) -> Result<Command> {
        // An explicit path in settings.json always wins:
        //
        //   "lsp": { "bgraph-lsp": { "binary": { "path": "…/bgraph-lsp" } } }
        if let Ok(settings) =
            zed::settings::LspSettings::for_worktree(language_server_id.as_ref(), worktree)
            && let Some(binary) = settings.binary
            && let Some(path) = binary.path
        {
            return Ok(Command {
                command: path,
                args: binary.arguments.unwrap_or_default(),
                env: worktree.shell_env(),
            });
        }

        let Some(path) = worktree.which(SERVER) else {
            return Err(format!(
                "`{SERVER}` was not found on PATH.\n\n\
                 Install it with `cargo install --path .` from the bgraph repository, \
                 or point Zed at it directly in settings.json:\n\n\
                 \"lsp\": {{ \"bgraph-lsp\": {{ \"binary\": {{ \"path\": \"/path/to/{SERVER}\" }} }} }}"
            ));
        };

        Ok(Command {
            command: path,
            args: Vec::new(),
            env: worktree.shell_env(),
        })
    }
}

zed::register_extension!(BgraphExtension);
