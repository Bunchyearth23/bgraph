// The extension is a thin launcher: everything semantic — diagnostics,
// completion, hover, go-to-definition, outline — comes from `bgraph-lsp`,
// which is the same server the Zed extension speaks to and the same front half
// of the compiler the CLI runs. Highlighting is the TextMate grammar beside
// this file; VS Code needs no parser of its own for that.

const { workspace, window } = require('vscode');

let client;

function activate() {
  const command = workspace.getConfiguration('bgraph').get('server.path', 'bgraph-lsp');

  // Required inside `activate`, not at the top of the file, so a package built
  // without `node_modules` still activates and still highlights. Failing to
  // load the module at import time would take the whole extension down and
  // leave a `.bd` file looking like plain text for a reason nobody could see.
  let LanguageClient;
  let TransportKind;
  try {
    ({ LanguageClient, TransportKind } = require('vscode-languageclient/node'));
  } catch (error) {
    window.showWarningMessage(
      'bgraph: highlighting only — this build has no language client, so there are no ' +
        'diagnostics or completion. Rebuild the package after `npm install`.',
    );
    return;
  }

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

// ---------------------------------------------------------- markdown preview

/// A ```bgraph fence, rendered in the Markdown preview.
///
/// The single largest reason Mermaid gets used instead of something better:
/// a diagram you can only see by leaving the document is one you stop keeping
/// current.
///
/// It renders through the **wasm module bundled beside this file**, not
/// through a `bgraph` on `PATH`. A preview that worked only for people who had
/// already installed the CLI would miss exactly the readers it is for. The
/// module is instantiated synchronously — `markdown-it`'s fence rule has no
/// way to await — which the non-streaming `WebAssembly.Module` constructor
/// allows in the extension host.
let render;

function loadRenderer() {
  if (render !== undefined) return render;
  try {
    const fs = require('fs');
    const path = require('path');
    const bytes = fs.readFileSync(path.join(__dirname, '..', 'bgraph.wasm'));
    const instance = new WebAssembly.Instance(new WebAssembly.Module(bytes), {});
    const api = instance.exports;
    const encoder = new TextEncoder();
    const decoder = new TextDecoder();

    render = (source) => {
      const input = encoder.encode(source);
      const ptr = api.bgraph_alloc(input.length);
      // A fresh view every call: the module's memory may have grown, and a
      // view onto a detached buffer throws rather than reading stale bytes.
      new Uint8Array(api.memory.buffer, ptr, input.length).set(input);
      const at = api.bgraph_render(ptr, input.length);
      const len = api.bgraph_result_len();
      const answer = decoder.decode(new Uint8Array(api.memory.buffer, at, len));
      api.bgraph_free(ptr, input.length);
      return answer;
    };
  } catch (err) {
    // No module bundled, or it failed to load. The fence falls back to a code
    // block, which is what it was before — a preview that threw would take the
    // whole document's rendering with it.
    render = null;
  }
  return render;
}

function escapeHtml(text) {
  return text.replace(/[&<>"]/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' })[c]);
}

function extendMarkdownIt(md) {
  const fallback = md.renderer.rules.fence.bind(md.renderer.rules);

  md.renderer.rules.fence = (tokens, index, options, env, self) => {
    const token = tokens[index];
    if ((token.info || '').trim().split(/\s+/)[0] !== 'bgraph') {
      return fallback(tokens, index, options, env, self);
    }

    const draw = loadRenderer();
    if (!draw) return fallback(tokens, index, options, env, self);

    const answer = draw(token.content);
    // One channel, so there is nothing to forget to check: an error announces
    // itself in the text the renderer returned.
    if (answer.startsWith('error:')) {
      return `<pre class="bgraph-error">${escapeHtml(answer)}</pre>`;
    }
    return `<div class="bgraph">${answer}</div>`;
  };

  return md;
}

module.exports.extendMarkdownIt = extendMarkdownIt;
