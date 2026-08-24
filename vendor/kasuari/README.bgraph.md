# Kasuari adaptation for bgraph

This directory starts from Kasuari 0.4.12, distributed under MIT OR
Apache-2.0. The upstream source and both licence texts are retained here.

bgraph's `D-045` requires identical coordinates across processes and targets.
Upstream Kasuari deliberately uses hash-table iteration as a fast tie breaker,
and documents that an underconstrained system may therefore choose different
valid solutions. This adaptation changes the tableau, variable maps, edit map
and changed-variable set to ordered collections. The constraint lookup remains
hashed because it is never iterated and therefore cannot select a pivot.

Keep the diff against the upstream 0.4.12 tag small. Any update must rerun the
cross-process determinism test and the native/no-default/wasm/WASI/Typst gate.
