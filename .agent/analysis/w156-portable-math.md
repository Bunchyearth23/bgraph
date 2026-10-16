# W-156 — portable transcendental geometry

## Observed differences and source provenance

The initial `/tmp/bgraph-native-linux-x64.json` compared with
`/tmp/bgraph-native-local/linux-arm64.json` had 58 raw-geometry differences;
the Windows MSVC/Wine artifact had 61. The 58 shared differences were confined
to `example/block`, `example/class` and `example/network`. Their node sizes
were identical; the affected linear positions did not involve transcendental
math. The Linux executable predated W-153's final insertion order.

Rebuilding the current Linux default-feature snapshot produced
`/tmp/bgraph-native-local/linux-x64-current.json`: **52 cases**, three runs in
process and three fresh processes, each repeated three times. Comparing common
case IDs against the older 51-case artifacts found **zero ARM differences**
and exactly **three Windows differences**, all the y coordinate of point 11
of a sampled BPMN circle:

| Case | Linux platform-math bits | Windows platform-math bits |
|---|---|---|
| `example/bpmn` | `4003a9fd8650dc8a` | `4003a9fd8650dc89` |
| `fixture/bpmn-cycle` | `403a3c6bf19f6de3` | `403a3c6bf19f6de2` |
| `fixture/bpmn-dependent` | `4057536cc521d435` | `4057536cc521d434` |

The new minimum-conflict fixture was absent from those older ARM/Windows
artifacts; it was not silently treated as comparable. No bit correction was
made for the 58 stale-executable differences.

## Choice and implementation

The remaining input is `sin(TAU * 11 / 32)`. Rust's platform transcendental
functions have unspecified precision, unlike correctly rounded `sqrt` and
the exact rounding methods. See the primary
[Rust f64 documentation](https://doc.rust-lang.org/std/primitive.f64.html#method.sin).
Using a special 32-point table would fix only the observed samples, leaving
arbitrary pie/radar angles and routing calculations platform dependent.

The approved implementation adds **`libm = "=0.2.16"`**, disables its default
features and enables **`force-soft-floats`**. This overrides both intrinsic and
architecture-specific implementations, including feature unification from
other consumers. This behavior is documented in the primary
[libm manifest](https://docs.rs/crate/libm/0.2.16/source/Cargo.toml.orig) and
[configuration implementation](https://docs.rs/crate/libm/0.2.16/source/configure.rs).
The crate has no production dependencies.

Private `src/math.rs` exposes only the functions actually used: sin, cos,
hypot, pow, log10, and feature-dependent acos/tan. Chart, BPMN, SVG routing and
PDF geometry call those portable algorithms. Squaring uses an explicit
multiplication rather than the unspecified-precision `powi` API. Primitive
IEEE arithmetic, correctly rounded `sqrt`, and precise floor/ceil/round are
unchanged. The browser UI's JavaScript `Math.exp` only controls viewport zoom;
it does not generate diagram geometry.

This chooses one complete algorithm implementation for all inputs; it adds no
coordinate quantization, precision reduction, diagnostic filtering, runtime
exception or change to snapshot comparison. The dependency and lock are pinned,
and the six-direct-dependency promise replaces the previous five in README,
AGENTS, Cargo comments and dependency-test comments. The native harness and
goldens were not edited for this correction.

## Local evidence

- `CARGO_TARGET_DIR=target/solver-work cargo check --lib`: exit 0,
  `/tmp/bgraph-portable-math-check.log`.
- `CARGO_TARGET_DIR=target/solver-minimal cargo check --no-default-features --lib`:
  exit 0, `/tmp/bgraph-portable-math-minimal.log`.
- `CARGO_TARGET_DIR=target/solver-minimal cargo tree --no-default-features --depth 1`:
  exactly six direct dependencies, including libm 0.2.16;
  `/tmp/bgraph-portable-math-dependencies.log`.
- `CARGO_TARGET_DIR=target/solver-work cargo test --lib math::tests`: two tests
  passed, `/tmp/bgraph-portable-math-unit.log`. Exact raw-bit vectors include
  the failing circle input, a large-angle reduction, signed zero, cos, hypot,
  pow, log10, and optional acos/tan. A source/config regression guard forbids
  reintroducing platform transcendental calls or changing the selected library
  version/features unnoticed.
- `CARGO_TARGET_DIR=target/solver-work cargo clippy --all-targets --all-features -- -D warnings`:
  exit 0, `/tmp/bgraph-portable-math-clippy.log`.
- `CARGO_TARGET_DIR=target/solver-work cargo test --lib --all-features` after
  adding both math regressions passed **689 tests, zero failed, one ignored**
  in 8.35 s; `/tmp/bgraph-portable-math-final-suite.log`.
- `cargo fmt --all --check`: exit 0,
  `/tmp/bgraph-portable-math-fmt.log`; `git diff --check` also passed.

Current portable Linux snapshot command:

```sh
CARGO_TARGET_DIR=target/solver-work \
BGRAPH_NATIVE_RUNNER=ubuntu-latest \
BGRAPH_NATIVE_SNAPSHOT=/tmp/bgraph-native-local/linux-x64-portable.json \
cargo test --test native_parity native_snapshot -- --exact --nocapture
```

It exited 0: **52 cases**, three in-process runs, three fresh processes, in
2.62 s. Log: `/tmp/bgraph-portable-math-native.log`. Exhaustive comparison with
the current Linux snapshot before libm finds **exactly the three raw-bit
changes above**, now equal to the previous Windows results. **Every SVG byte
and diagnostic is unchanged**. No gallery/golden regeneration was necessary.
Recorded differences: `/tmp/bgraph-portable-math-deltas.json`.

This local comparison proves the cause and correction without claiming that
older ARM/Windows artifacts were built with libm. Their final recompilation,
additional macOS/Windows ARM native execution and browser/WASI/Typst parity
belong to the coordinator's W-156/W-160 final-source gates. No remote CI
execution, commit or publication is implied by these local results.

Graft saved approximately **448,536 tokens across five retrieval calls** in
this investigation.

## Frozen-source feature and performance recheck

The final Cargo manifest was parsed with Python `tomllib`; the `all-archetypes`
list contains 29 individual archetype features. With
`CARGO_TARGET_DIR=target/pipeline-feature-checks`, the following were executed
sequentially, requiring an exit code of zero from each:

```sh
cargo check --locked --no-default-features --lib
cargo check --locked --no-default-features --lib --features <each of 29 archetypes>
```

**All 30 configurations passed** in 87.333 s on the frozen sources including
the portable math implementation. The log records every exact command, feature,
exit code and duration: `/tmp/bgraph-goal-features-final.log`.

The final `cargo bench --locked --bench pipeline -- --check` also exited zero
without budget increases. Log `/tmp/bgraph-goal-benchmark-final.log`; complete
per-call means are 1.116 s for 20,000 connected nodes and 222.29 ms for the
public `render_source` pipeline with 5,000 nodes, each measured twice. The
remaining cases and original ten-run guards all pass. Full final-source
performance observations are appended to `w153-completion.md`.

## Actual remote native proof

The five-platform gate has now executed successfully at
`90163b769f8be9a125424eba3ba4694ee4a1e967` in
[run 38895893757](https://github.com/Bunchyearth23/bgraph/actions/runs/38895893757).
Real Linux x86_64/ARM64, Windows x86_64/ARM64 and macOS ARM64 jobs all passed,
and their aggregate compared all 52 cases exactly. Independent audit verified
GitHub's archive digests, revision/default-feature metadata, every repetition
count, source/SVG/raw-bit/diagnostic equality and the actual provider jobs.
Full evidence is in `native-ci-proof.json` and the remote-execution section of
`w156-native-proof.md`. This supersedes the earlier lack of macOS and Windows
ARM native execution evidence. The overall workflow remains failed on two
separate font-dependent tests; complete W-160 CI proof awaits the corrected run.
