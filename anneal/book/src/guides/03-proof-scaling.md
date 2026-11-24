<!-- Copyright 2026 The Fuchsia Authors

Licensed under a BSD-style license <LICENSE-BSD>, Apache License, Version 2.0
<LICENSE-APACHE or https://www.apache.org/licenses/LICENSE-2.0>, or the MIT
license <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your option.
This file may not be copied, modified, or distributed except according to
those terms. -->

# Discharging and Scaling Scalar Proofs

As scalar functions grow beyond a single bitwise instruction, copying low-level
MIR transitions inside every theorem becomes fragile. This guide shows how to
separate domain models from execution proofs, reuse `Anneal.MirWp` stepping
lemmas, and factor multi-module proofs cleanly in Stage 0.

> **Alpha software:** This is alpha software! Stage 0 verification remains
> restricted to call-free, pointer-free scalar code (`Profile v1` on
> `x86_64-unknown-linux-gnu`) with out-of-line `Spec.lean` and `Proof.lean`
> modules. Complete review-surface reporting and an independent TCB audit
> remain under active development; see
> [Alpha Usage](https://github.com/google/zerocopy/blob/main/anneal/docs/alpha.md)
> for current Stage 0 limits.

## 1. Keep `Spec.lean` focused on the domain model

Consider a capability flag sanitizer in `lib.rs` that forces required flags and
masks out disallowed flags:

```rust
{{#include ../../examples/guides/03-proof-scaling/lib.rs}}
```

In `Spec.lean`, define a pure reference function `expectedFlags` first, then
state both exact behavior (`respectsPolicy`) and a downstream security
invariant (`clearsDisallowed`) in terms of pure bitvectors:

```lean
{{#include ../../examples/guides/03-proof-scaling/Spec.lean.txt}}
```

Splitting the pure function from behavioral boilerplate keeps specification
review compact even when multiple invariants are attached to the same body.

## 2. Factor `Proof.lean` into three layers

Rather than proving each invariant by stepping MIR independently, structure
`Proof.lean` (or split helper files listed in `request.json`) in three layers:

1. **Symbolic MIR execution once** (`executes_sanitize`): step the MIR body
   once using `Anneal.MirWp` lemmas (`step_u64Use`, `step_u64Or`, `step_u64And`)
   to connect entry state directly to `Spec.expectedFlags`.
2. **Valid-argument elimination once** (`entryArgs_elim`): unwrap
   `ValidEntryArgs` and scalar truncation invariants via
   `checkArgsMatch_cons_iff` and `checkVal_int_iff` in a reusable helper.
3. **Pure domain algebra separately** (`cleared_by_mask`): discharge
   `(out &&& ~~~allow) = 0#64` on pure `BitVec 64` values with
   `BitVec.eq_of_getLsbD_eq`, never mentioning MIR blocks or locals.

```lean
{{#include ../../examples/guides/03-proof-scaling/Proof.lean.txt}}
```

For larger crates, you can split helper lemmas into separate proof modules in
`request.json` and import `Anneal.BitVec` or `Anneal.LoopInv` from the bundled
proof library (see [`theory/bundle-manifest.json`][bundle-manifest]).

[bundle-manifest]: https://github.com/google/zerocopy/blob/main/anneal/theory/bundle-manifest.json

```json
{{#include ../../examples/guides/03-proof-scaling/request.json}}
```

## 3. Verify and catch subtle refactoring bugs

Running `cargo anneal verify --request request.json` verifies both the exact
specification and the cleared-bit security property cleanly:

```console
{{#include ../../examples/guides/03-proof-scaling/pass-transcript.txt}}
```

Now introduce a subtle precedence refactor bug in `lib.rs` that forgets to mask
existing `flags`:

```rust
{{#include ../../examples/guides/03-proof-scaling/broken-lib.rs}}
```

Without touching `Spec.lean`, rerunning `verify` fails closed in the `audit`
phase because unmasked bits in `flags` violate both `respectsPolicy` and
`clearsDisallowed`:

```console
{{#include ../../examples/guides/03-proof-scaling/fail-transcript.txt}}
```

During local proof refactoring, you can temporarily unblock downstream modules
with `--allow-sorry-in-proof` (never permitted in `Spec.lean`), remembering that
any `--allow-*` flag taints the receipt (`"tainted_selected_unit_verified"` or
`"tainted_crate_verified"`, adding the flag's taint atom even when no `sorry`
remains) for as long as the flag is passed. Next, see
[Direct Compilation with `verify rustc`](04-verify-rustc-fuchsia.md) for
integrating Anneal directly into non-Cargo build systems such as Fuchsia GN or
Bazel.
