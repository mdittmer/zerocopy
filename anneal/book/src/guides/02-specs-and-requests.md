<!-- Copyright 2026 The Fuchsia Authors

Licensed under a BSD-style license <LICENSE-BSD>, Apache License, Version 2.0
<LICENSE-APACHE or https://www.apache.org/licenses/LICENSE-2.0>, or the MIT
license <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your option.
This file may not be copied, modified, or distributed except according to
those terms. -->

# Specs, Requests, and Profile v1

In Stage 0 alpha, Anneal keeps all verification metadata out of your Rust
source tree: there is no proc-macro crate, no build script hook, and no inline
specification comment syntax inside Rust files yet. Instead, a crate carries an
out-of-line `Spec.lean` specification, a `Proof.lean` proof script, and a
`request.json` manifest (or direct CLI/build-system arguments).

> **Alpha software:** This is alpha software! Stage 0 supports only the
> call-free, pointer-free scalar MIR fragment (`Profile v1` on
> `x86_64-unknown-linux-gnu`) and refuses constructs outside that boundary with
> `"status": "unsupported"`. Complete review-surface reporting and an
> independent TCB audit remain under active development; review both
> `Spec.lean` and `request.json` before trusting a passing receipt. See the
> [Alpha Usage and Profile v1 Limits][alpha-doc] guide for the full
> supported/refused table.

[alpha-doc]: https://github.com/google/zerocopy/blob/main/anneal/docs/alpha.md

## 1. The Rust target (`lib.rs`)

Consider a minimal scalar permission helper in `lib.rs` that clears any
requested bit not permitted by `allowed_mask`:

```rust
{{#include ../../examples/guides/02-specs-and-requests/lib.rs}}
```

Because `Spec.lean` and `Proof.lean` live out-of-line, downstream crates that
depend on `perm_mask` do not need to install Anneal or depend on Lean.

## 2. Writing `request.json`

`request.json` pins the compilation inputs, verification scope, and Lean module
roles that Anneal passes to the Phase 1 and Phase 2 checks:

```json
{{#include ../../examples/guides/02-specs-and-requests/request.json}}
```

Three fields matter most to human reviewers:

- `compilation`: binds `crate_name`, `crate_root`, `source_root`, and every
  declared file in `source_files` (relative non-symlink paths only). Optional
  `edition`, `target`, `panic`, `cfg`, and `env` keys specify exact compiler
  flags.
- `scope`: selects either `{"kind": "selected_unit", "roots": [...]}` (with
  optional `"instantiations"` for explicit concrete monomorphizations) or
  `{"kind": "crate"}` across a whole dependency-free, feature-free crate.
- `proof.sources`: marks `Spec` as `"role": "specification"` (trusted and
  human-reviewed) and `Proof` as `"role": "proof"` (untrusted, kernel-checked).

For dependency-free, feature-free single-target Cargo crates, `cargo anneal
verify --manifest-path Cargo.toml --proof-dir proof` builds the same request
from Cargo metadata automatically.

## 3. Writing `Spec.lean` honestly

Specification modules (`"role": "specification"`) live inside Anneal's reviewed
trust boundary. `Spec.lean` can import Lean standard modules (`Init.*`,
`Std.*`) and `Trusted.*` semantics definitions, while Phase 1 `"spec-preflight"`
restricts specification modules from pulling in untrusted proof tactics or
unapproved axioms:

```lean
{{#include ../../examples/guides/02-specs-and-requests/Spec.lean.txt}}
```

Notice two Stage 0 design habits in `Spec.lean`:

- Declare `specs : Trusted.MirSemantics.Specs` with `safety` (caller
  preconditions for any `unsafe` target) and `property` (functional behavior on
  `entryProgForBody program 0`).
- Omit unused parameter bindings: if `matchesPermissionMask` took an unused
  `(body : Body)` argument, Anneal's anti-vacuity checker would emit an
  `"unused_spec_binding"` warning in `spec_warnings` on stdout and stderr.

## 4. Discharging `Expected.claim` in `Proof.lean`

`Proof.lean` imports `Expected` (generated from `Spec.lean` + captured MIR)
and `Anneal.MirWp` to quantify over all `2^128` input pairs `(requested,
allowed)` symbolically without enumerating cases:

```lean
{{#include ../../examples/guides/02-specs-and-requests/Proof.lean.txt}}
```

## 5. Run `verify` and test a mutation

Running `cargo anneal verify --request request.json` confirms
`"status": "selected_unit_verified"` with `taint_reasons: []` and
`spec_warnings: []`:

```console
{{#include ../../examples/guides/02-specs-and-requests/pass-transcript.txt}}
```

Dropping the mask in `lib.rs` simulates a real privilege-check bug:

```rust
{{#include ../../examples/guides/02-specs-and-requests/broken-lib.rs}}
```

Rerunning `cargo anneal verify --request request.json` without changing
`Spec.lean` fails closed in the `audit` phase:

```console
{{#include ../../examples/guides/02-specs-and-requests/fail-transcript.txt}}
```

As functions grow beyond single statements, see
[Discharging and Scaling Scalar Proofs](03-proof-scaling.md) for patterns that
keep `Proof.lean` modular and maintainable.
