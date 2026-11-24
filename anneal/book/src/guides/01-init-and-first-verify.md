<!-- Copyright 2026 The Fuchsia Authors

Licensed under a BSD-style license <LICENSE-BSD>, Apache License, Version 2.0
<LICENSE-APACHE or https://www.apache.org/licenses/LICENSE-2.0>, or the MIT
license <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your option.
This file may not be copied, modified, or distributed except according to
those terms. -->

# Your First Standalone Proof

This guide walks through creating and verifying a self-contained Stage 0 scalar
proof using `cargo anneal init --standalone` and `cargo anneal verify`.

> **Alpha software:** This is alpha software! Stage 0 verifies only a small,
> call-free, pointer-free scalar subset of Rust (`Profile v1` on
> `x86_64-unknown-linux-gnu`), its Trusted Computing Base (TCB) audit and
> complete review-surface reporting remain under active development, and passing
> runs depend on an intact host and human review of `Spec.lean`,
> `request.json`, and the TCB receipt. See
> [Installing the Toolchain](../getting-started/installation.md) to set up the
> pinned toolchain before running these steps locally.

## 1. Scaffold the standalone files

Create an empty directory and ask `cargo anneal init --standalone` to generate
a four-file starter proof bundle:

```console
{{#include ../../examples/guides/01-standalone/init-transcript.txt}}
```

Every file written into `hello/` serves a distinct role:

### `hello/lib.rs` (implementation)

The Rust target is ordinary, dependency-free code without inline proof macros
or specification comments:

```rust
{{#include ../../examples/guides/01-standalone/lib.rs}}
```

### `hello/request.json` (compilation and scope bindings)

The JSON request binds the Rust crate root, the verified scope (`zero`), and
the Lean modules (`Spec` as `"specification"`, `Proof` as `"proof"`):

```json
{{#include ../../examples/guides/01-standalone/request.json}}
```

### `hello/Spec.lean` (trusted specification)

Human reviewers inspect `Spec.lean` to confirm what Anneal proves. Here `specs`
requires `zero()` to terminate safely without diverging and return `0u64`:

```lean
{{#include ../../examples/guides/01-standalone/Spec.lean.txt}}
```

### `hello/Proof.lean` (untrusted proof)

`Proof.lean` discharges `Expected.claim` using symbolic/kernel execution helpers
from `Anneal.MirWp`. Because Lean 4's kernel checks every step of
`Proof.verify`, reviewers do not need to audit the proof tactics:

```lean
{{#include ../../examples/guides/01-standalone/Proof.lean.txt}}
```

## 2. Run `cargo anneal verify`

Verify the scaffolded request and inspect the Receipt Schema v3 output
(condensed below for brevity; live runs also emit `audit`, `census`, `sources`,
digests, and timing metrics, and `elaboration_sandbox` records `linux_bwrap` or
`linux_landlock_seccomp` on Linux, shown as
`<linux_bwrap | linux_landlock_seccomp>` below):

```console
{{#include ../../examples/guides/01-standalone/pass-transcript.txt}}
```

Confirm three key fields on every passing run:

- `status`: `"selected_unit_verified"` (exit code `0`) confirms every declared
  specification obligation holds for the selected `zero` unit.
- `taint_reasons`: `[]` confirms zero `--allow-*` relaxation flags were passed.
- `spec_warnings`: `[]` confirms no anti-vacuity warnings fired on `Spec.lean`.

To inspect the generated Lean proof obligation (`Expected.lean`) or captured
MIR (`Captured.lean`), pass `--artifacts <new-directory>` to `cargo anneal
verify` (the target directory must not already exist).

## 3. Confirm the proof catches a real bug

Replace `hello/lib.rs` with a buggy implementation that returns `1` instead of
`0`:

```rust
{{#include ../../examples/guides/01-standalone/broken-lib.rs}}
```

Rerunning `cargo anneal verify --request hello/request.json` now fails closed
with exit code `1` and `"status": "proof_failed"` in the `audit` phase:

```console
{{#include ../../examples/guides/01-standalone/fail-transcript.txt}}
```

Notice that `Proof.lean` is flagged rather than `Spec.lean`: because the
specification stayed fixed while the implementation drifted, Anneal rejects the
broken code rather than weakening the contract. For writing your own scalar
specifications and requests from scratch, continue to
[Specs, Requests, and Profile v1](02-specs-and-requests.md).
