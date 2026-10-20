# Modularization task list

Objective: reduce the responsibilities of `src/lsp.rs` without changing
language-server behavior. This is a historical implementation record.

- [x] Establish a baseline: `cargo test --all-targets` — 555 tests pass.
- [x] Extract the UTF-8/UTF-16 index into `src/lsp/positions.rs`.
- [x] Extract JSON-RPC framing and capabilities into `src/lsp/protocol.rs`.
- [x] Extract diagnostics and notification publishing into `src/lsp/diagnostics.rs`.
- [x] Extract completion and snippets into `src/lsp/completion.rs`.
- [x] Extract hover, symbols, definitions and references into `src/lsp/navigation.rs`.
- [x] Extract lexical context and declaration helpers into `src/lsp/context.rs`.
- [x] Move tests to their relevant modules while keeping server integration tests.
- [x] Check after each extraction: `cargo test --all-targets`, `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`.
- [x] Check reduced builds: `cargo build --no-default-features` and relevant combinations (`pie`, `packet,json,csv,pdf`, `wasm`, `png`).
- [x] Measure module size and dependencies before considering `W-141` complete.

Implementation order: `positions` → `protocol` → `diagnostics` →
`completion` → `navigation` → `context`.

## Five implementation batches prepared at the time

### 1. Completion and snippets — `src/lsp/completion.rs`

- Scope: `completion`, `items_for`, `completion`, `snippet`, `item_snippet` and completion helpers.
- Dependencies retained: `Context`, `declared`, `declared_archetype`, `item_words`, `props`.
- Guard: preserve suggestion order, filtering and content.
- Exit criterion: completion tests pass, including strings/comments, template holes, styles, aliases and snippets.

### 2. Navigation — `src/lsp/navigation.rs`

- Scope: hover, document symbols, go-to-definition, references and document highlights.
- Dependencies retained: `LineIndex`, `diagnostics`, `word_at`, `declaration_span`, `text_of`.
- Guard: preserve UTF-16 ranges and `null` responses when no target exists.
- Exit criterion: identical JSON responses in hover, symbol, definition and reference tests.

### 3. Lexical context and declarations — `src/lsp/context.rs`

- Scope: `Block`, `Context::at`, item words, declarations, identifiers, directions and lexical helpers.
- Dependencies retained: `props`, `ast::Span` and indirectly `LineIndex`.
- Guard: remain lexical when a file is invalid; do not introduce a second parser.
- Exit criterion: context, group completion and archetype-word tests remain green.

### 4. Tests and cleanup — `src/lsp/*` and `src/tests.rs`

- Move unit tests beside their module; keep cross-server contract tests in `src/tests.rs`.
- Remove unused imports and make internal interfaces explicit (`pub(super)` or `pub(crate)`).
- Add a regression test for each extracted module before deleting its old implementation.
- Exit criterion: no test relies on a private detail merely because it remained in `lsp.rs`.

### 5. Final validation — features, size and dependencies

- Rerun `cargo test --all-targets`, `cargo fmt --check` and strict Clippy.
- Rerun `cargo build --no-default-features`, then `pie`, `packet,json,csv,pdf`, `wasm` and `png` combinations where supported.
- Compare `wc -l src/lsp.rs`, `cargo tree --no-default-features` and new-module dependencies before/after.
- Exit criterion: identical behavior, no warnings and no unnecessary dependency introduced by extraction.
