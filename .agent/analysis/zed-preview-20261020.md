# Zed browser preview — W-170 / D-054

Date: 2026-10-20. Changes remain uncommitted; no user installation or profile change.

## Result

The extension supplies the native language task `bgraph: Preview` in
`editors/zed/languages/bgraph/tasks.json`. The optional entry in
`editors/zed/keymap.json` binds it to `Ctrl+Shift+V` in `.bd` editors. Users
merge that entry into their existing keymap; extensions do not install global
keybindings themselves. The preview is in the default browser and reloads on
save, using the existing watch server.

The task saves the current buffer and invokes `bgraph watch <file> --open
--port 0` from the file's directory. `--open` is new: it opens the actual
bound URL after successful binding. Port zero chooses a free loopback port.
The existing public `watch::serve` API keeps its signature and behavior.
An unavailable or failing browser opener leaves a working server and prints
the URL. Directory build loops reject `--open` explicitly.

`editors/zed/install.sh` now prepares both CLI binaries when explicitly run.
It was not run during this work. The pre-existing grammar revision edit in
`extension.toml` was preserved.

## Observed host evidence

Real Zed **1.24.3+stable**, Xvfb, private D-Bus and disposable profile:
`/tmp/bgraph-zed-host-v1uewyc3`. Durable result snapshot:
[`zed-preview-host-20261020.json`](zed-preview-host-20261020.json).

- Zed loaded the component and grammar and launched the real language server.
- The task was discovered from the extension's language directory. No global
  or project task copy supplied it during the successful run.
- The real `Ctrl+Shift+V` binding saved an unsaved edit before invoking the CLI.
- A filename containing spaces, an apostrophe, `$()`, backticks and `$file`
  reached the CLI as one unchanged argument; no `INJECTED` marker was created.
- The browser opener received the actual nonzero loopback URL. Its `/svg`
  response rendered the chosen source; saving new content changed `/version`
  and the served drawing.
- Repeating the shortcut caused two invocations over time but left **one
  active watcher**, with the newest URL serving the preview.

The opener in the isolated host is a recording stub. The test verifies the
opening request and actual served page, not a graphical default browser.

## Compatibility finding

On this Zed host, `args: ["watch", "$ZED_FILE", ...]` splits a path containing
spaces. Directly interpolating quoted filenames into a shell command would
also interpret filename contents. The task instead binds `BGRAPH_PREVIEW_FILE`
through Zed's native `env` field, then expands `"$BGRAPH_PREVIEW_FILE"` in the
shell. Shell expansion does not reinterpret commands inside a variable's
value. The hostile-name host test proves the complete chain.

`Editor && extension == bd` is the supported keymap context. The initially
considered `language == bgraph` context is absent from Zed's editor contexts.

The filename expression was independently checked under installed `sh`,
`bash`, `fish` and `zsh`. The README gives the simple PowerShell argument
override; Windows and macOS native browser opening were not executed.

Opening the single file alone on this host did not expose the language task;
opening its containing folder plus the file did. The README therefore makes
opening the folder an explicit setup step, and the successful host check uses
that supported project context. The unsuccessful standalone-file evidence is
in `/tmp/bgraph-zed-host-xtk04z_e`.

## Validation

| Check | Observed result |
|---|---|
| `cargo test --locked --test watch_preview` | 5 passed |
| `cargo clippy --locked --bin bgraph --test watch_preview -- -D warnings` | passed |
| `cargo build --locked --no-default-features` | passed |
| Zed extension `cargo build --locked --target wasm32-wasip1` | passed |
| `rustfmt --edition 2024 --check src/main.rs src/watch.rs tests/watch_preview.rs` | passed |
| `bash -n editors/zed/install.sh` | passed |
| Real Zed host `--preview` | passed; evidence above |
| Default real Zed LSP host | passed; `/tmp/bgraph-zed-host-6iqbbrhf` |

Integration cases cover actual port selection and saving/reload, occupied
ports before browser opening, missing or unsuccessful openers, the directory
restriction, and synchronized task/keymap configuration. The default Zed LSP
host mode remains separate from the preview mode.

## Remaining practical limits

The keymap entry needs merging into the user's configuration, and the updated
CLI/extension need installation. Open the containing folder in Zed so the
native language task is available. Included files use their last saved contents.
The provided task uses Linux/macOS shell syntax; the documented PowerShell
override has not run on a Windows host. A browser tab from an older watcher
may need closing after a restart opens the new URL.
