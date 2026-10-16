# LSP editing corrections — 2026-10-16

User report: `(0, 0)` rejected as non-numeric, node completions inside layout,
insufficient typing assistance in Zed.

## Observations before changes

- Zed launches `/home/bunchy/.cargo/bin/bgraph-lsp`; its hash differed from
  the built release server, and its timestamp was older.
- Zed's log contained repeated `server shut down` errors.
- The supplied source, copied to `/tmp/bgraph-user-layout.bd` with `(0, 0)`,
  produced no diagnostics in either CLI binary. Opening that source over LSP
  likewise produced an empty diagnostic array with both existing servers.
  The reported numeric diagnostic was not reproduced; zero is valid.
- Source inspection found a cursor-prefix bug: a final opening brace used
  the full document length to determine its offset, misidentifying its owner
  or slicing inside a final multibyte character.
- Layout snippets inserted non-numeric `x`/`y` defaults and invalid square
  brackets around row/column gap options. Partial position words had no
  completion branch, and local scope depended on the whole document parsing.

## Changes

- Track the actual brace offset, active statement and container lexically;
  keep coordinate commas, strings, comments and template holes distinct.
- Complete inline blocks and incomplete local layouts with scoped entity
  choices; exclude self anchors and repeated row members.
- Use numeric snippet defaults, valid row/column syntax, position snippets,
  gap/alignment options and direction-appropriate alignment values.
- Supply explicit completion replacement ranges, preserving hyphenated words,
  and refresh lists as the prefix changes.
- Add layout signature help for coordinates, relative placements and members.
- Expose the shared formatter through LSP document formatting.

## Evidence and remaining work

`cargo build --locked --bins` and `cargo clippy --locked --lib -- -D warnings`
succeeded. No test suite was added or run for this request.
The updated release server is installed at the existing Zed launch path.
Restart the language server in Zed to load the new executable and capabilities;
interactive Zed behaviour after that restart remains to be confirmed.
