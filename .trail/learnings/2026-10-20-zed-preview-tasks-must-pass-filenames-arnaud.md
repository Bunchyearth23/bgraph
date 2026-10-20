---
title: Zed preview tasks must pass filenames through native environment fields
author: Arnaud
date: 2026-10-20
branch: main
touches:
  - AGENTS.md
  - docs/INDEX.md
  - docs/goldens.txt
  - docs/guide/commandes.md
  - docs/guide/editeurs.md
  - docs/png-goldens.txt
  - docs/reference.md
  - docs/release-notes/1.0.0.md
  - editors/README.md
  - editors/vscode/README.md
---

## Decided
The bundled native Zed task lives beside languages/bgraph/config.toml. It saves the current buffer and runs bgraph watch --open --port 0. The optional Ctrl+Shift+V keymap context is Editor && extension == bd; Zed does not expose a language key in the editor context.

## Tried and ruled out
On the actual Zed 1.24.3 host, args containing a bare $ZED_FILE split a filename with spaces. Quoting an interpolated filename directly risks interpreting command substitutions in that filename. Use env BGRAPH_PREVIEW_FILE:$ZED_FILE, then expand the environment value inside quotes in args; do not put a source path into shell code. Native language tasks do work; a disappearing task with a hostile filename did not establish a packaging limitation.

## Watch out
The real isolated --preview host test in tests/runtime/zed-host.py checks task discovery, the actual shortcut, saving before launch, filenames with spaces/apostrophe/$()/backticks, URL rendering/reload, and one active watcher after repeat. Two execs can still mean one active server because Zed replaces the old terminal. The default provided task uses Linux/macOS shell syntax (also Fish); PowerShell needs the documented $env:BGRAPH_PREVIEW_FILE argument. Opening the browser is checked with a recording opener, not a graphical browser. The server prints/opens listener.local_addr(), not the requested port zero, and browser failures must leave that server working.
