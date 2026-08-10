// The extension is a thin launcher: everything semantic — diagnostics,
// completion, hover, go-to-definition, outline — comes from `bgraph-lsp`,
// which is the same server the Zed extension speaks to and the same front half
// of the compiler the CLI runs. Highlighting is the TextMate grammar beside
// this file; VS Code needs no parser of its own for that.

const { workspace, window } = require('vscode');
const { LanguageClient, TransportKind } = require('vscode-languageclient/node');

let client;

function activate() {
  const command = workspace.getConfiguration('bgraph').get('server.path', 'bgraph-lsp');

  client = new LanguageClient(
    'bgraph',
    'bgraph',
    {
      run: { command, transport: TransportKind.stdio },
      debug: { command, transport: TransportKind.stdio },
    },
    {
      documentSelector: [{ scheme: 'file', language: 'bgraph' }],
      synchronize: { fileEvents: workspace.createFileSystemWatcher('**/*.bd') },
    },
  );

  // A missing binary is the one failure a user cannot diagnose from an empty
  // editor, so it is reported rather than swallowed.
  client.start().catch((error) => {
    window.showErrorMessage(
      `bgraph: could not start \`${command}\` — install it with \`cargo install --path .\`, ` +
        `or set \`bgraph.server.path\`. (${error.message})`,
    );
  });
}

function deactivate() {
  return client ? client.stop() : undefined;
}

module.exports = { activate, deactivate };
