// The extension is a thin launcher: everything semantic — diagnostics,
// completion, hover, go-to-definition, outline — comes from `bgraph-lsp`,
// which is the same server the Zed extension speaks to and the same front half
// of the compiler the CLI runs. Highlighting is the TextMate grammar beside
// this file; VS Code needs no parser of its own for that.

const { workspace, window, commands, ViewColumn } = require('vscode');
const path = require('path');
const { previewHtml } = require('./preview');

let client;
const previews = new Map();

function activate(context) {
  context.subscriptions.push(
    commands.registerCommand('bgraph.showPreview', showPreview),
    workspace.onDidChangeTextDocument(({ document }) => {
      const preview = previews.get(document.uri.toString());
      if (!preview) return;
      clearTimeout(preview.timer);
      preview.timer = setTimeout(() => updatePreview(preview), 150);
    }),
    workspace.onDidCloseTextDocument((document) => {
      previews.get(document.uri.toString())?.panel.dispose();
    }),
    { dispose: disposePreviews },
  );
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
      'bgraph: this build has no language client, so there are no ' +
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
      documentSelector: [
        { scheme: 'file', language: 'bgraph' },
        { scheme: 'untitled', language: 'bgraph' },
      ],
      synchronize: { fileEvents: workspace.createFileSystemWatcher('**/*.{bd,bds,csv,json}') },
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
  disposePreviews();
  return client ? client.stop() : undefined;
}

module.exports = { activate, deactivate };

// ----------------------------------------------------------- diagram preview

async function showPreview(uri) {
  const document = uri ? await workspace.openTextDocument(uri) : window.activeTextEditor?.document;
  if (!document || document.languageId !== 'bgraph') {
    window.showInformationMessage('bgraph: open a .bd file to preview it.');
    return;
  }
  if (path.extname(document.fileName).toLowerCase() === '.bds') {
    window.showInformationMessage('bgraph: this is a shared stylesheet. Open a .bd diagram that includes it to preview the result.');
    return;
  }
  const key = document.uri.toString();
  let preview = previews.get(key);
  if (preview) {
    preview.panel.reveal(ViewColumn.Beside, true);
  } else {
    const panel = window.createWebviewPanel(
      'bgraph.preview',
      `Preview: ${path.basename(document.fileName)}`,
      { viewColumn: ViewColumn.Beside, preserveFocus: true },
      { enableScripts: true, localResourceRoots: [] },
    );
    preview = { document, panel, timer: undefined };
    previews.set(key, preview);
    panel.webview.onDidReceiveMessage((message) => {
      if (message?.type !== 'ready') return;
      preview.ready = true;
      if (preview.message) panel.webview.postMessage(preview.message);
    });
    panel.onDidDispose(() => {
      clearTimeout(preview.timer);
      previews.delete(key);
    });
    panel.webview.html = previewHtml();
  }
  updatePreview(preview);
}

function updatePreview(preview) {
  clearTimeout(preview.timer);
  let source = '';
  let answer;
  try {
    source = preview.document.getText();
    const draw = loadRenderer();
    answer = draw ? draw(source) :
      'error: the preview renderer is missing or could not load. Rebuild the extension with package.sh.';
  } catch (error) {
    answer = `error: could not render this diagram. ${error.message}`;
  }
  const name = preview.document.fileName || 'diagram.bd';
  // The compiler's own wording for a diagram that reads another file names a
  // Rust function — `bgraph::render_file` — which nobody in an editor can
  // call. The same condition is stated here in terms the reader can act on.
  // The wording is pinned by a test, so a change in `src/lib.rs` fails loudly
  // rather than quietly restoring the unusable message.
  if (/needs a file to resolve it against/.test(answer)) {
    answer = compositionError(source, name, untitled(preview.document));
  }
  preview.message = answer.startsWith('error:') ?
    { type: 'render', error: answer } : { type: 'render', svg: answer };
  // A pie that reads its slices from a CSV is refused by nothing and drawn
  // wrongly: the renderer ignores the file and draws what the buffer happens
  // to declare. The picture is kept — it is not wrong about the rest of the
  // diagram — and the caveat is shown with it.
  const notice = answer.startsWith('error:') ? null :
    externalDataNotice(source, name, untitled(preview.document));
  if (notice) preview.message.notice = notice;
  if (preview.ready) preview.panel.webview.postMessage(preview.message);
}

// A buffer that has never been saved has no file to watch.
function untitled(document) {
  return document.uri?.scheme === 'untitled';
}

// The first `include` in the source, and the file it names.
const INCLUDE_STATEMENT = /^[ \t]*include[ \t]+"([^"]*)"/m;

// `type pie { from: "sales.csv" }` — the archetype block that reads its
// slices from a table. The value has to be a quoted file name: `from:` on an
// edge names a side (`from: right`) and must not be mistaken for this.
const EXTERNAL_DATA = /^[ \t]*type[ \t]+[A-Za-z_][A-Za-z0-9_-]*\s*\{[^}]*\bfrom[ \t]*:[ \t]*"[^"]*"/m;

function compositionError(source, name, isUntitled) {
  const include = INCLUDE_STATEMENT.exec(source);
  const which = include ? `\`${include[1]}\`` : 'another file';
  return `error: this diagram includes ${which}, and the preview renders the current buffer only.\n` +
    (isUntitled ?
      'Save it as a file, then run `bgraph watch <file>` to see the composed diagram.' :
      `Save it, then run \`bgraph watch ${name}\` to see the composed diagram.`);
}

function externalDataNotice(source, name, isUntitled) {
  if (!EXTERNAL_DATA.test(source)) return null;
  return 'This diagram reads its data from another file, which the preview cannot open. ' +
    'The picture shows only what this buffer declares. ' +
    (isUntitled ?
      'Save it as a file, then run `bgraph watch <file>` to see the data.' :
      `Save it, then run \`bgraph watch ${name}\` to see the data.`);
}

function disposePreviews() {
  for (const { panel } of previews.values()) panel.dispose();
}

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
      try {
        new Uint8Array(api.memory.buffer, ptr, input.length).set(input);
        const at = api.bgraph_render(ptr, input.length);
        const len = api.bgraph_result_len();
        return decoder.decode(new Uint8Array(api.memory.buffer, at, len));
      } finally {
        api.bgraph_free(ptr, input.length);
      }
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
