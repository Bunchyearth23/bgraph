# English documentation and first-use workflow

Date: 2026-10-20. The user requested documentation usable by someone who has
never seen bgraph, then required the documentation to be entirely English.

## Result

- Rewrote the README and all 12 task guides in English. The entry route is
  installation, a complete first diagram, then optional editor support.
- Defined the source/output distinction, command-line tools, PATH, terminal
  working folders and identifiers versus visible labels. Instructions say
  where to run commands and what the result should look like.
- Added a first-use sequence covering validation, SVG/PDF export, live
  editing, stopping the watcher and deliberately diagnosing an unknown node.
- Documented VS Code's unsaved-buffer pane and Zed's saved-file browser task,
  including local extension installation and the Zed folder/keymap requirement.
- Translated site navigation, theme/search language, generated gallery text
  and launcher help. Existing guide URLs are retained.
- Updated the binary-export README template with installation and first-use
  steps that do not require a source checkout.
- Corrected retired inline placement in the generated reference's source,
  vocabulary design note and printable manual. Added editor previews to the
  generated reference and manual. Regenerated the reference through Cargo;
  it was not edited by hand.
- Added a first-file page to the printable manual, fixed its incomplete
  bytefield/orgchart examples and rebuilt the 22-page A4 PDF.
- Translated operational index/work lists without changing existing IDs,
  states, dates or row order. D-055 records the English beginner-documentation
  policy. Historical 0.1.0 release notes retain their original syntax with
  an explicit historical notice and a link to the current tutorial.

Archived session evidence and user tooling configuration are preserved.

## Observed verification

| Check | Result |
|---|---|
| Complete README/guide diagram fences rendered with the CLI | 17 passed |
| Complete printable-manual examples rendered separately | Eight passed |
| First-use source validation, SVG/PDF/JSON export and intentional unknown-node error | Passed |
| Watch command with `--open --port 0` | Actual URL delivered to a recording opener; served SVG changed after a saved edit |
| Composition, component, CSV and template commands | Seven passed |
| Export README template executed in isolation | Passed, version and first-preview command checked |
| Documentation/reference/capability and editor-guide Rust tests | Nine targeted tests passed |
| Reference regeneration | `cargo run --locked --bin bgraph -- docs -o docs/reference.md`, exit 0 |
| Strict MkDocs build | Passed |
| Generated site crawler | 18 HTML pages, 1,124 local links/assets and anchors passed |
| Search configuration and gallery assets | English only; 30 source/image pairs |
| Real browser | Home, installation, tutorial, editor guide, search results and 30 gallery source links checked |
| Printable manual | Typst compilation passed; all 22 pages rendered to a contact sheet, changed teaching/editor pages and final cover inspected |
| Shell syntax, launcher help, Rust formatting and whitespace | Passed |
| Index invariants | Existing ID/state/date prefixes and ordering retained; only D-055/X-032 added |

The first composition smoke check exposed an actual resolver limitation;
the final guide includes the verified workaround below. An earlier checker
also counted shell fences as data files; filtering by fence language fixed
that checker, without a product change.

Metadata: [documentation-english-20261020.json](documentation-english-20261020.json).
Site screenshot: [documentation-english-20261020.png](documentation-english-20261020.png).
The compiled manual is `out/manual.pdf`, a generated, ignored build output.

## Shared-style include limitation — X-032

Given `house.bds` declaring `.service`, and `application.bd` including that
file and using the class, including only `application.bd` under `as backend`
reports unknown `.service`. Including `house.bds` at the root as well makes
the same composed diagram render successfully. This workaround is now in
the composition tutorial and troubleshooting; X-032 is mitigated, with a
future resolver correction still needed. No compiler behavior was changed
as part of this documentation update.

## Practical limits

Installer instructions were checked against local scripts, manifests and
the official Rust installation page. They were not replayed as installations
into the user's profile or on native Windows/macOS systems. The full
cross-platform export was not rebuilt; its README template was verified
independently. Existing editor packages were not reinstalled.

The site is built locally and has not been deployed. Validation was performed
before the documentation and Venn commit, based on implementation commit
87c7536. The user subsequently requested commit and push; Git records delivery.
