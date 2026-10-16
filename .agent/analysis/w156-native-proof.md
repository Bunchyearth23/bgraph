# W-156 / W-160 — native determinism evidence

## Implemented contract

`tests/native_parity.rs` runs `native_snapshot` with the default features. Its
catalog contains the 30 worked sources in the compiler's `examples::EXAMPLES`
table and 22 regression fixtures, for **52 cases**. No gallery files are read.
The fixtures cover declared, grouped, managed, composed, underconstrained and
fractional layouts; nested frames, grid frames and external anchors; overlap,
unknown anchors/components, Unicode diagnostics, alignment/size/frame conflicts;
and BPMN attachment dependents, explicit conflicts and consistent/inconsistent
attachment cycles.

Sources are included in the evidence and normalized from CRLF to LF before
analysis. Package version, source revision, build profile, enabled surface
features, actual target OS/architecture and runner label are recorded. Metadata
about the machine is separate from the semantic snapshot that must compare equal.

For each case the artifact contains:

- The exact SVG string, whose UTF-8 representation is the rendered SVG bytes;
  no coordinate, whitespace, Unicode or decimal normalization is applied.
- Every diagnostic's severity, UTF-8 byte span and complete message, together
  with the render error string when rendering fails.
- Unrounded `f64::to_bits` as 16 hexadecimal digits, preserving signed zero:
  node/group rectangles and numeric resolved styles; edge waypoints; guide
  endpoints/widths; polyline vertices/widths; annotation positions/sizes;
  routed edge vertices and label rectangles; canvas shift/size; graph/grid
  geometry; and the complete pie center/radii/slice angles. `Chart` is matched
  exhaustively so adding a new variant cannot silently omit its geometry.

Each parent computes its full snapshot **three times**, then starts **three fresh
copies of its integration-test executable**. Each child computes the snapshot
three times too and reports through an isolated temporary file. All comparisons
are exact; a mismatch reports the first JSON pointer. Time, temporary paths and
process identifiers are not part of the compared data.

`BGRAPH_NATIVE_CHILD_RUNNER` is an optional JSON argv array prepended to the child
executable, supporting e.g. `qemu-aarch64 -L /usr/aarch64-linux-gnu`. It is passed as
argv directly, without a shell. The child guard prevents recursive spawning.
Windows uses `CreateProcess` through the standard library; an output path under
Wine must be supplied using its Windows spelling, e.g. `Z:\\tmp\\snapshot.json`.

## CI provisioning

`.github/workflows/ci.yml` runs the native snapshot on five explicit targets:

| Runner | Rust target |
|---|---|
| ubuntu-latest | x86_64-unknown-linux-gnu |
| ubuntu-24.04-arm | aarch64-unknown-linux-gnu |
| windows-latest | x86_64-pc-windows-msvc |
| windows-11-arm | aarch64-pc-windows-msvc |
| macos-latest | aarch64-apple-darwin |

The triplet is explicit for builds and execution, preventing an x86 toolchain
installed on an ARM runner from silently producing an emulated x86 proof.
Each successful job uploads `native-snapshot-v1-<runner>-<commit>` for 14 days;
a missing file is an upload error. `workflow_dispatch` is available.

The aggregate job runs even after a failed upstream matrix job. It requires
exactly five artifacts, one from each runner, validates the actual OS/architecture,
schema, source revision, default feature set, complete case catalog and repetition
counts, then compares all sources, SVG bytes, diagnostics and geometry bits.
It rejects missing, duplicate, malformed, incomplete and unequal evidence.
There is no mode that declares a partial platform set successful.

The runner labels and private-repository availability were verified against
[GitHub's hosted-runner reference](https://docs.github.com/en/actions/reference/runners/github-hosted-runners)
and the [January 29, 2026 ARM availability announcement](https://github.blog/changelog/2026-01-29-arm64-standard-runners-are-now-available-in-private-repositories/).
The reference identifies `macos-latest` as ARM64 for private repositories too.

## Executed evidence

- `BGRAPH_NATIVE_SNAPSHOT=/tmp/bgraph-native-linux-x64.json BGRAPH_NATIVE_RUNNER=ubuntu-latest CARGO_TARGET_DIR=target/pipeline-work cargo test --locked --test native_parity -- native_snapshot --exact --nocapture`
  — **exit 0** on Linux x86_64; 51 cases, 3 in-process snapshots and 3 fresh
  processes, each with 3 in-process snapshots. Final schema run: 14.74s test
  execution after compilation. The artifact is 1,132,749 bytes.
- `node tests/runtime/compare-native.mjs --self-test` — **exit 0**. A complete
  synthetic five-platform fixture is accepted; 18 deliberately changed or
  incomplete fixtures are rejected, including a raw signed-zero bit change,
  changed SVG/source/diagnostic, missing platform/case/geometry, wrong feature,
  profile, version, revision and missing fresh-process evidence.
- The aggregate comparator was also run with the real Linux artifact alone;
  it exited **1** with `exactly five native artifacts are required`. The wrapper
  asserted this failure, demonstrating that local Linux evidence is not enough
  to make the multi-platform gate green.
- The CI YAML was parsed with Python/PyYAML; its five unique matrix entries and
  mandatory aggregate dependency were asserted successfully.
- `node --check tests/runtime/compare-native.mjs` and `git diff --check` — exit 0.

## Reproduction on another target

Build without executing and capture Cargo's JSON artifact records:

```sh
cargo test --locked --test native_parity --target <triplet> --no-run --message-format=json > /tmp/native-build.jsonl
```

Read the record with `reason == "compiler-artifact"`, `target.name ==
"native_parity"` and `"test"` in `target.kind`; `executable` is the exact test
binary path. This avoids guessing its fingerprinted filename. Execute it with
`--exact native_snapshot --nocapture`, setting `BGRAPH_NATIVE_SNAPSHOT` to the
output path and `BGRAPH_NATIVE_RUNNER` to the corresponding runner label.
For Linux ARM cross builds set
`CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=aarch64-linux-gnu-gcc` as appropriate.
For QEMU children use
`BGRAPH_NATIVE_CHILD_RUNNER='["qemu-aarch64","-L","/usr/aarch64-linux-gnu"]'`.
The local Wine execution and the CI Windows jobs both test MSVC binaries.

## Final local cross-target proof

Fresh default-feature target binaries completed **52 cases** with three in-process
snapshots and three fresh child processes, each repeated three times:

- Linux x86_64 native: `/tmp/bgraph-native-local/linux-x64-portable.json`.
- Linux ARM64 under QEMU: `/tmp/bgraph-native-local/linux-arm64-final.json`,
  `/tmp/bgraph-native-local/linux-arm64-final.log`, **exit 0**.
- Windows x86_64 **MSVC** under Wine: `/tmp/bgraph-native-local/windows-x64-final.json`,
  `/tmp/bgraph-native-local/windows-x64-final.log`, **exit 0**.

Python compared the entire snapshot object exactly, including source catalog,
SVG bytes, all raw f64 bits, diagnostics, features and profile. **All three are
identical**. Result and artifact paths: `/tmp/bgraph-native-local/parity-final.json`.
This is an additional partial platform proof, not a successful five-platform
CI result. The strict official comparator still requires all five real runners.
Its final self-test accepts a synthetic complete set and rejects 18 corrupt
or incomplete sets; it was rerun after adding the 52nd fixture.

The comparison found and corrected a real Windows sine discrepancy with pinned
portable math. Implementation, initial stale-binary diagnosis and exact bit
changes: `w156-portable-math.md`. No SVG or diagnostic changed from that math fix.

The final portable-source Windows ARM64 MSVC CLI and LSP also cross-build
successfully (`/tmp/bgraph-goal-windows-arm64-final.log`, exit 0). This adds
compilation evidence only; it is not native Windows ARM execution.

## Historical local-only limit — superseded by the remote proof below

QEMU and Wine execute the target binaries locally; they do not prove real
Windows, native ARM operating systems, macOS or hosted CI. The five-platform
workflow has not been pushed, dispatched or executed. GitHub has no authenticated
host and no macOS host was supplied. W-156/W-160 remain unfinished until actual
native execution evidence exists. No commit or publication was performed.

Graft: 5 retrieval calls, approximately 271,941 source-reading tokens saved.

## Actual five-platform CI proof — independently audited

The native execution requirement is now proven on GitHub-hosted runners at
commit **`90163b769f8be9a125424eba3ba4694ee4a1e967`**,
[CI run 38895893757](https://github.com/Bunchyearth23/bgraph/actions/runs/38895893757).
All five native jobs succeeded, followed by the successful
[exact aggregate comparison](https://github.com/Bunchyearth23/bgraph/actions/runs/38895893757/job/116957844648).

| Real runner | Reported target OS / architecture | Successful job ID | Artifact ID |
|---|---|---:|---:|
| ubuntu-latest | Linux / x86_64 | 116956144216 | 12043678261 |
| ubuntu-24.04-arm | Linux / aarch64 | 116956144363 | 12043997139 |
| windows-latest | Windows / x86_64 | 116956144479 | 12044231612 |
| windows-11-arm | Windows / aarch64 | 116956144388 | 12043329488 |
| macos-latest | macOS / aarch64 | 116956143968 | 12043513421 |

Independent audit retrieved job/provider metadata and all five original ZIP
archives by artifact ID. Every archive's SHA-256 matches GitHub's recorded
digest, and its JSON matches the separately downloaded artifact bytes. Each
document matches the revision, compiler version, default feature set, debug
profile, expected target OS/architecture and its provider runner label. Each
proves **three in-process runs and three fresh child processes**, with **three
runs per child**. The complete catalog has **52 cases: 30 examples and 22
fixtures, including 42 valid and 10 invalid inputs**.

The official strict comparator was rerun locally with `GITHUB_SHA` set to the
commit above and all five real artifacts: **exit 0**. Independent Python
comparison of every entire snapshot object also passed. Source text, SVG bytes,
all raw geometry bits and diagnostics agree without quantization or tolerance.
The remote aggregate additionally passed all 18 rejection self-tests.

Machine-readable evidence, including artifact archive/file checksums, all job
IDs, provider runner IDs and shared source/snapshot/geometry/diagnostic hashes:
**`native-ci-proof.json`**. The shared canonical snapshot SHA-256 is
`3c7d8a73b2090d64e3788a07dc68f035ba51ec412371996a06ee92021ea00fb4`.

Audit logs/artifacts:

- `/tmp/bgraph-ci-artifacts-38895893757-final/`
- `/tmp/bgraph-ci-native-aggregate-38895893757.log`
- `/tmp/bgraph-ci-native-independent-audit.log`

**This proves native W-156 parity at that revision; it does not make the
overall workflow successful.** Its separate `check` job failed on
`a_subset_font_is_smaller_and_still_a_font` (NotoColorEmoji did not shrink)
and `representative_png_snapshots_are_byte_stable`. Those failures are recorded
in `/tmp/bgraph-ci-check-failure-38895893757.log` and the machine proof.
W-160's complete CI requirement still needs the subsequent corrected run.

This independent audit used one graft retrieval, approximately 3,623 tokens saved.
