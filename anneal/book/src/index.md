<!-- Copyright 2026 The Fuchsia Authors

Licensed under a BSD-style license <LICENSE-BSD>, Apache License, Version 2.0
<LICENSE-APACHE or https://www.apache.org/licenses/LICENSE-2.0>, or the MIT
license <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your option.
This file may not be copied, modified, or distributed except according to
those terms. -->

# Anneal

Anneal is a formal verification framework for Rust backed by Lean 4. It proves
that Rust programs—including `unsafe` code—are free of undefined behavior (UB)
and satisfy developer-defined specifications, with Lean 4's kernel checking every
proof while keeping human review bounded to specifications and the trusted
computing base (TCB).

> **This is alpha software! (Stage 0)** Anneal is in alpha and under active
> development; APIs, specification syntax, and verification coverage change
> frequently, and its Trusted Computing Base (TCB) audit and complete
> review-surface reporting remain in progress. Today it checks only a small,
> call-free, pointer-free scalar subset of Rust (`Profile v1` on
> `x86_64-unknown-linux-gnu`), and passing runs depend on an intact build host
> (`rustc`, `build.rs`, and proc-macros run outside the Lean sandbox) as well
> as human review of specifications (`Spec.lean`), compilation/scope inputs
> (`request.json` or `verify rustc` flags), and TCB audit logs. The current CLI
> ships `cargo anneal setup` (versioned toolchain archive installation),
> `cargo anneal init` (starter `Spec.lean` and `Proof.lean` scaffolding or a
> runnable `--standalone` example), `cargo anneal verify` (including
> `--request`, `--manifest-path`/`--proof-dir` for dependency-free library or
> binary crates with optional `--panic`, `--select`, and `--instantiate`, and
> `verify rustc` for build-system integration using out-of-line `Spec.lean` and
> `Proof.lean` modules), and `cargo anneal skill` (bundled verification
> guidance and a working starter example). Inline Rust specifications and
> broader MIR coverage are under development. To keep this book accurate, every
> command and code example shown here is executed against the current
> `cargo-anneal` build in CI alongside static surface-drift checks for CLI
> flags, bundled theory libraries, skills, and pinned toolchains. See
> [Alpha Usage and Profile v1 Limits](https://github.com/google/zerocopy/blob/main/anneal/docs/alpha.md)
> on GitHub for the exact supported and fail-closed refused Rust subset.

## What Anneal promises

Anneal's guarantee rests on an explicit division of labor between automated
verification and bounded human review (see
[Project Principles](foundations/principles.md) and the
[Threat Model](foundations/threat-model.md)):

- **If** verification runs in an uncompromised execution environment (such as an
  isolated CI runner),
- **a reviewer agrees** with the crate's **stated specifications** (what you
  asked Anneal to prove) and its **TCB audit log** (the trusted code and
  unproved assumptions Anneal relied on), and
- **Anneal emits no errors**,
- **then** the verified code is sound (UB-free) and satisfies its stated
  specifications.

Everything outside those two surfaces—proofs, proof automation, and verified
Rust implementation bodies that do not define specifications or TCB
assumptions—can be safely ignored by human reviewers.

## Current CLI surface

Every command transcript in this book is checked directly against the live
`cargo-anneal` binary in CI:

```console
{{#include ../examples/cli/help/transcript.txt}}
```

## Where to go next

- [Installing the Toolchain](getting-started/installation.md) walks through
  `cargo anneal setup`, local archives, cache cleanup, and fail-closed archive
  handling.
- [Your First Standalone Proof](guides/01-init-and-first-verify.md),
  [Specs, Requests, and Profile v1](guides/02-specs-and-requests.md),
  [Discharging and Scaling Scalar Proofs](guides/03-proof-scaling.md), and
  [Direct Compilation with `verify rustc`](guides/04-verify-rustc-fuchsia.md)
  walk through Stage 0 workflows with live-checked Lean proofs and mutations.
- [CLI Reference](reference/cli.md) documents `setup`, `init`, `skill`,
  `verify`, `verify rustc`, Receipt Schema v3 diagnostics, and CLI error
  handling.
- [Project Principles](foundations/principles.md),
  [Threat Model](foundations/threat-model.md), and
  [Meta-Principles](foundations/meta-principles.md) define Anneal's correctness
  promise and trust boundaries.
- Contributing to Anneal itself? See the developer documentation on GitHub:
  [Alpha Usage](https://github.com/google/zerocopy/blob/main/anneal/docs/alpha.md),
  [Developing Anneal](https://github.com/google/zerocopy/blob/main/anneal/docs/README.md),
  [Design Overview](https://github.com/google/zerocopy/blob/main/anneal/docs/design.md),
  [Architecture](https://github.com/google/zerocopy/blob/main/anneal/docs/architecture.md),
  and
  [Trust Model](https://github.com/google/zerocopy/blob/main/anneal/docs/trust-model.md).
