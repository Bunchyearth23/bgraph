---
title: Editor previews fit both axes and LSP inspection shares live include buffers
author: Arnaud
date: 2026-10-22
branch: main
touches:
  - ".github/workflows/ci.yml"
  - AGENTS.md
  - docs/API.md
  - docs/INDEX.md
  - docs/goldens.txt
  - docs/guide/commandes.md
  - docs/guide/composition.md
  - docs/guide/editeurs.md
  - editors/README.md
  - editors/vscode/README.md
---

## Decided
VS Code preview fits the SVG viewBox against both the visible pane width and height, with no upscaling of small diagrams. HTML is created once; renders are posted after a ready handshake. Fit mode recenters on resize; manual zoom/pan survives edits, errors and webview recreation through VS Code state. The command has an icon, keyboard and pointer controls. `.bds` is recognized in VS Code and Zed, and VS Code sends untitled bgraph buffers to the LSP. Navigation, references and structural hover use the same open-source snapshot as diagnostics; hover analyzes the existing composition once.

## Tried and ruled out
Width-only SVG CSS did not constrain tall drawings. Replacing webview HTML per keystroke loses the camera. A passing host lifecycle check did not prove geometry: measure actual SVG rectangles in the real webview. VS Code freezes its public API, so monkeypatching window.createWebviewPanel silently fails. The test harness intercepts require('vscode') only for extension.js and uses Object.create to preserve proposed-API getters. Its nonce-authorized DOM probe exists only in the isolated test profile.

## Watch out
The current-buffer WASM preview still rejects includes and external data; keep its documented watch fallback. LSP tests must cross dirty included files with UTF-16 positions and didClose disk restoration, including a file buffer that has never been saved. Wheel magnitudes differ between platforms; use only the sign. Build the VSIX after editing src/preview.js. The 2026-10-22 follow-up passed 798 Rust and 12 extension JS tests, strict Clippy/minimal build, plus real Linux Code and Zed hosts; Windows/macOS and screen readers remain unverified. No user-profile installation or publication was performed.
