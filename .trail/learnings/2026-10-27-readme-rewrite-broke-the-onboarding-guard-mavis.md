---
title: README rewrite broke the onboarding guard; re-pin it to the README
author: mavis
date: 2026-10-27
branch: main
id: l-8d359346
scope: repo
status: active
touches:
  - "src/tests.rs#the_readme_onboarding_uses_supported_commands_and_examples"
  - docs/INDEX.md
---

## Decided

The `3567c31` README rewrite (release landing page) is deliberate, so the
onboarding guard follows the README rather than the other way round
(user decision, `D-057`). `the_readme_onboarding_uses_supported_commands_and_examples`
now pins the commands the README actually shows (`cargo install --locked --path . --bins`,
`bgraph --version`, `bgraph check architecture.bd`, the SVG/PDF exports,
`bgraph watch architecture.bd --open --port 0`, `bgraph init pie -o chart.bd`) and
parses the README's own fenced ```bgraph block instead of three copied snippets —
so the example half of the guard can no longer drift from the file it guards.

## Tried and ruled out

- Restoring `bgraph init free`, `--theme light|dark|print` and `type pie` to the
  README: offered as the first option, the user chose to re-pin the test instead.
- Copying the README example into the test again (what the old test did): that is
  exactly what let the two drift, so the block is now read from the file.

## Watch out

- CI was red on `main` for two runs before this: `39776360015` (`3567c31`) and
  `39781674796` (`49ac2ed`, HEAD), both failing only this test in `check`.
  `39764410364` (`b32c82f`) passed. A green run after the fix is the remaining proof.
- The guard reads fenced blocks from the file, so an unclosed ```bgraph fence in the
  README panics the test rather than failing an assertion.
- `trail check` reports seven lexical conflicts on these files ("brings back full
  run", "readme example", "bgraph file", "test pass"). They are word overlaps with
  learnings that *instruct* running those things — reviewed and unrelated.
