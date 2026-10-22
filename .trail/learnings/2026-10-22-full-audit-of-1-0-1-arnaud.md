---
title: Full audit of 1.0.1 found gaps across templates composition and delivery
author: Arnaud
date: 2026-10-22
branch: main
touches:
  - AGENTS.md
  - editors/zed/extension.toml
  - opencode.json
  - export.sh
---

## Decided
- The 2026-10-22 audit is a report only; no product fixes, commits, publication or installation were requested. Full editable report: https://chatgpt.com/space/page_24e4faa833608191bed9b99f4e1399ca (20 points). Audited HEAD 79d2d8094bef9e14128ceb9dd7f56f2dded86f0a; preserve pre-existing tooling/AGENTS/Zed manifest changes.
- Prioritize four demonstrated failures: export.sh deletes its destination before a successful build; CLI output can overwrite its own source; template bindings bypass output-format/theme/font handling and always write SVG; a missing field in one JSON row can silently remove that row from a pie.
- Other reproductions: component expansion drops notes and unsupported nested declarations; prefixed includes miss note/legend/waypoint anchors; template slot inference misquotes names; CSV readers disagree and duplicate columns overwrite values; watch freshness misses external CSV and directory include changes; LSP parent diagnostics remain stale after dependency edits; idle TCP blocks watch; PDF replaces unrepresentable text and accepts non-finite/zero page sizes.
- Isolated script tests prove Zed installation can absorb unrelated staged files or miss an already-staged grammar, and WASM/Typst scripts can copy old target/ artifacts under CARGO_TARGET_DIR. Windows file-URI conversion and keyboard accessibility findings are static, not verified in a real Windows host or screen reader. Playground startup overwrite was tested on its actual callback with delayed loading; Undo remains available.

## Tried and ruled out
- 751 Rust tests pass by default; 760 pass with png, each with one manual performance test ignored. Minimal build, all-target/all-feature strict Clippy, fmt, six VS Code JS tests and runtime gate (22 fixtures, 30 types, four file contexts, extensions included) pass. Passing these does not cover the failed cross-feature and state-transition cases.
- Do not report a current 512-entity solver fallback: W-153 removed it. Do not call standalone WASM preview's lack of filesystem composition a new defect; it is documented.
- A 2000-file include chain crashes debug but passes release; the 10000-file release case reports a parser stack limit. No release crash claim is supported.

## Watch out
- X-005 font measurement remains a known open issue: embedding does not recompute layout advances. Reused the 2026-10-21 evidence rather than claiming a new visual reproduction.
- Keep reproductions involving deletion, commits and packaging in temporary copied repositories with simulated build tools. Product sources remain unchanged; fix requests must add contract tests rather than merely parsing the same happy path again.
- No fresh native Windows/macOS, real editor-host, dependency vulnerability or exhaustive performance audit was run. The detailed report distinguishes direct reproductions, static reasoning and product improvements.
