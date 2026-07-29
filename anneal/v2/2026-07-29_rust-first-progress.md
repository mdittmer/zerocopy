# Anneal v2 Rust-First Specifications

Date: 2026-07-29

Branch: `v2-oda__rust-lang-aeneas-fwk`

This branch starts with the feature set from `v2-oda__aeneas-fwk`, but makes
Rust the default language for executable conditions and computational proof
lemmas. Quoted Lean propositions, Lean proof fragments, and auxiliary Lean
theorems remain available as escape hatches.

## Rust Conditions

Import the attribute macro and write condition expressions directly in Rust:

```rust
use anneal_macros::anneal;

#[anneal(spec(
    requires(
        name = "flag_true",
        prop = flag == true,
    ),
    ensures(
        name = "pass_thru",
        prop = ret == flag,
    ),
))]
pub fn needs_true(flag: bool) -> bool {
    flag
}
```

The scanner accepts both the imported spelling `#[anneal(...)]` and the
canonical qualified spelling `#[anneal_macros::anneal(...)]`. Renaming the
procedural macro on import is not currently supported. Each expansion leaves
an invisible HTML-comment marker on the original item. After Charon runs,
Anneal reconciles those markers by selected crate root and full logical Rust
owner. Renamed, `macro_rules!`-generated, and `include!`-generated Anneal
attributes therefore fail closed with a targeted error instead of silently
losing a contract or theorem. Conditionally compiled annotated items are also
rejected until the source scanner models `cfg` alternatives. Local
`#[charon::exclude]`/`#[aeneas::exclude]` items are rejected as well because
they would remove the expansion marker from LLBC.

The macro emits ordinary, hidden Rust Boolean helper functions. For this
example their Rust paths are:

```text
anneal__needs_true::flag_true
anneal__needs_true::pass_thru
```

Charon translates those helpers with the rest of the crate, and Aeneas emits
ordinary `Result Bool` Lean definitions. Anneal's generated hypotheses and
postconditions refer directly to those lowered definitions using
total-correctness specifications of the form:

```lean
anneal__needs_true.flag_true flag ⦃ holds => holds = true ⦄
```

Consequently a condition is not treated as true if its Rust computation
returns `false`, fails, panics, encounters undefined behavior, or diverges.
Anneal does not generate a second Lean `def` containing a transcription of the
Rust expression.

Named conditions use their names for helper paths and generated hypothesis
names. Unnamed conditions receive stable role-local names such as `pre1`,
`inv1`, and `post1`. Postconditions may use the reserved `ret` binding.
Struct invariants may use `self`.

The same model covers:

- free-function preconditions, postconditions, and invariants;
- required trait-method contracts inherited by concrete implementations; and
- invariants on structs with private named fields.

Rust-authored conditions are currently restricted to the non-generic function
and struct shapes for which Anneal's LLBC theorem model can render binders.
Direct conditions on impl methods and default trait methods are rejected
because an attribute macro cannot determine their enclosing item context.
Put a shared method contract on the required trait declaration instead.

## Rust Theorem Functions

A Boolean Rust function can be marked as a computational theorem:

```rust
#[anneal(
    theorem,
    spec(requires(name = "bounded", prop = x <= 10)),
    proof(auto),
)]
pub fn reflexive(x: u32) -> bool {
    x == x
}
```

Its generated soundness theorem requires the Rust function to terminate
successfully with `true`; explicit `ensures` and invariants are conjoined with
that implicit result. The theorem is registered as an Aeneas `@[step]`
theorem so other generated proofs can use it. Omitting `proof(auto)` has the
same automatic behavior.

Rust theorem functions are initially limited to public, concrete free
functions returning exactly `bool`. Mutable-reference inputs and outputs are
rejected for all Rust-authored conditions and theorem functions until Anneal
models Aeneas' updated-reference result tuples in helper postconditions.

Anneal derives direct call edges from LLBC and emits automatically proved
`@[step]` theorems in callee-before-caller order. This includes calls made by
Rust condition helpers, so a condition can call a theorem function and a
theorem function can in turn call a specified ordinary function. An automatic
recursive dependency cycle is rejected with the participating Rust names;
an explicit Lean proof may break only the cyclic edges involving that proof.
Its ordinary acyclic call dependencies are retained, so the callee `@[step]`
theorems remain available.

Existing Lean escape hatches remain supported:

```rust
#[anneal(proof(lean::tactic = "simp_all"))]
```

and structured `theorem(...)` annotations continue to emit auxiliary Lean
theorem declarations. Raw Lean expressions, types, arguments, attributes, and
proof terms are delimiter-validated and emitted inside generator-owned
syntactic boundaries, so a fragment cannot terminate its declaration and
inject a top-level `axiom`.

Lean proof admissions (`sorry`, `admit`, and direct `sorryAx`) are rejected
unless the user passes `--allow-sorry`. Without that flag the generated module
also enables Lean's `warningAsError`, making Lean's semantic “declaration uses
sorry” warning the backstop for spellings the lexical pre-check cannot
anticipate. After Lake builds the result, Anneal runs a fixed checker which
loads each compiled `Soundness` module at runtime and rejects every axiom
declaration owned by that module, regardless of the namespace chosen by proof
metaprogramming. It also audits local declarations' transitive dependencies on
`sorryAx` when admissions are disabled. Trust-extending axiom declarations are
therefore never enabled by `--allow-sorry`. `--no-lean-build` explicitly skips
both proof compilation and this semantic audit.

The attribute syntax lives in the publishable `anneal-annotations` crate and
the procedural macro in `anneal-macros`; the CLI re-exports the same parser so
macro expansion and source scanning cannot silently drift.

## Source Provenance

The real procedural macro path is also the span-preserving path. Proposition
tokens are syntax-checked and classified as `syn::Expr` values while retaining
their original-spanned token stream, which is interpolated directly into the
helper body. They are never stringified and reparsed from text. Charon compiles
the original Cargo manifest, and its source table retains the pristine
original Rust file.

With the pinned Charon revision, statements lowered from an interpolated
proposition carry the original attribute-expression span directly; generated
scaffolding falls back to the attribute invocation span. This makes the
diagnostic provenance chain:

```text
original Rust expression
  -> proc-macro helper statement
  -> LLBC span and definition id
  -> Aeneas Lean definition
  -> generated Anneal theorem reference
  -> Lean diagnostic
```

The expansion marker is metadata on the original item, not an executable
helper. Its canonical token fingerprint lets the scanner prove that the
unexpanded item it parsed is the same annotation rustc expanded. Charon keeps
that marker with the item's original/macro span, giving future diagnostic
mapping an explicit original-item-to-expanded-item join. The marker is an
invisible rustdoc HTML comment; as a deliberate tradeoff, it also counts as
documentation for Rust's `missing_docs` lint.

Source scanning follows the selected Cargo target's Rust module graph,
including external modules and `#[path = "..."]` declarations. The selected
target source is always treated as the crate root, which keeps helper ownership
correct for nonstandard library roots and `src/bin/*.rs` targets.

The workspace-level `lean-error-rust-span-mapping-plan.md` contains the
complete mapping design and the derived-source fallback.
