<!-- Copyright 2026 The Fuchsia Authors

Licensed under a BSD-style license <LICENSE-BSD>, Apache License, Version 2.0
<LICENSE-APACHE or https://www.apache.org/licenses/LICENSE-2.0>, or the MIT
license <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your option.
This file may not be copied, modified, or distributed except according to
those terms. -->

# Direct Compilation with `verify rustc`

Build systems such as Bazel, GN, and Soong resolve source trees, compiler
flags, and module paths directly rather than calling Cargo. Anneal provides
`cargo anneal verify rustc` so those host-side CI and build rules can verify
Rust crates without writing an intermediate `request.json` file.

> **Alpha software:** This is alpha software! Stage 0 `verify rustc` covers
> only the call-free, pointer-free scalar MIR fragment (`Profile v1` on
> `x86_64-unknown-linux-gnu`) for dependency-free targets, and its TCB audit
> and complete review-surface reporting remain under active development. Do not
> rely on Stage 0 runs as a production security guarantee. See the
> [CLI Reference](../reference/cli.md) and
> [Alpha Usage][alpha-doc] for exact limits.

## 1. Multi-file crate layout

Consider a small width-masking crate modeled after register and eBPF helpers in
Fuchsia and Starnix, split across two Rust source files (`src/lib.rs` and
`src/width.rs`) and out-of-line Lean modules (`proof/Spec.lean` and
`proof/Proof.lean`):

### `src/lib.rs`

```rust
{{#include ../../examples/guides/04-verify-rustc-fuchsia/lib.rs}}
```

### `src/width.rs`

Stage 0 Profile v1 supports non-generic, inhabited, crate-local fieldless
enums (including explicit integer discriminants such as
`#[repr(u8)] pub enum DataWidth`):

```rust
{{#include ../../examples/guides/04-verify-rustc-fuchsia/width.rs}}
```

### `proof/Spec.lean`

`Spec.lean` inspects `DataWidth`'s captured enum layout via `body.locals`
(variant indices `0..3` follow `DataWidth`'s declaration order `U8` through
`U64` in `src/width.rs`) and requires `width::mask_width` to mask `value`
accurately for all four variants and all `2^64` words:

```lean
{{#include ../../examples/guides/04-verify-rustc-fuchsia/Spec.lean.txt}}
```

### `proof/Proof.lean`

`Proof.lean` combines `checkVal_enum_iff` and `checkVal_int_iff` from
`Anneal.MirWp` to eliminate `ValidEntryArgs` and verifies all four `match`
branches:

```lean
{{#include ../../examples/guides/04-verify-rustc-fuchsia/Proof.lean.txt}}
```

## 2. Invoking `cargo anneal verify rustc`

Pass the crate root (`src/lib.rs`) positionally alongside `--crate-name`,
every non-root Rust source (`--source-file src/width.rs`), `--select` (omitting
the crate prefix, e.g. `width::mask_width`), and `<MOD>=<PATH>` mappings via
`--spec-file` and `--proof-file`:

```console
{{#include ../../examples/guides/04-verify-rustc-fuchsia/pass-transcript.txt}}
```

Check how `verify rustc` fails when a branch regression drops the 32-bit mask
(`DataWidth::U32 => value`):

```rust
{{#include ../../examples/guides/04-verify-rustc-fuchsia/broken-width.rs}}
```

Rerunning the exact same command exits `1` with `"status": "proof_failed"` in
the `audit` phase:

```console
{{#include ../../examples/guides/04-verify-rustc-fuchsia/fail-transcript.txt}}
```

## 3. Build-system, Fuchsia, and toolchain notes

When wiring `cargo anneal verify rustc` into Bazel, GN, or host-test runners
(compare [`ci/verify_starnix_e2e.py`][starnix-e2e], which passes explicit
`--capture`, `--sysroot`, and `--lean` flags to `verify --request` over a
pinned ten-file `ebpf_core` manifest):

- **Explicit toolchain overrides (builds without `setup`):** Build systems
  that disable the default `setup` feature when compiling `cargo-anneal` supply
  `--capture`, `--sysroot`, and `--lean` explicitly. `cargo-anneal` rejects
  mismatches if `--capture` and captured `--sysroot` output fail to report
  Anneal's pinned `rustc` commit (`f8a08b688cbe`, `nightly-2026-05-31`, without
  hashing the sysroot tree itself) and verifies `--lean` against Anneal's
  `foundation-lock.json` SHA-256 (failing closed unless `--allow-unpinned-lean`
  taints the receipt).
- **Symlinked runfiles trees:** Unlike `--request`, `verify rustc` dereferences
  symlinks when staging declared inputs (still rejecting special files and
  concurrent edits) so Bazel/GN runfiles trees can be passed directly.
- **Retained evidence and `.olean*` export:** Pass `--artifacts <new-dir>`
  plus `--export-lean-lib` to collect retained `Captured.lean`,
  `Expected.lean`, and Door-1-audited `.olean*` artifacts under
  `<artifacts>/lean-lib/` when an action succeeds.
- **Downstream toolchain roll policy:** Today, rolling `rustc` or Lean requires
  an Anneal release that re-pins the compiler commit or `foundation-lock.json`,
  though verified Rust crates do not embed a fixed upstream schedule inside
  source annotations—downstream integrations such as Fuchsia or Starnix align
  `--capture`, `--sysroot`, and `--lean` updates with their own roll cadence
  against Anneal releases (or `--allow-unpinned-lean` tainted local runs while
  qualifying a new Lean build).

[alpha-doc]: https://github.com/google/zerocopy/blob/main/anneal/docs/alpha.md
[starnix-e2e]: https://github.com/google/zerocopy/blob/main/anneal/ci/verify_starnix_e2e.py
