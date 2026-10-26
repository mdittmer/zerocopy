prelude
import Init.Core
import Trusted
import Mir.ZerocopySlice.util_max
import Mir.ZerocopySlice.__anodized_fn_requires_util_max
import Mir.ZerocopySlice.__anodized_fn_ensures_util_max

namespace Obligations.ZerocopySlice.util_max

set_option linter.unusedVariables false

open Trusted.MirSemantics
open Trusted.VerificationResult

/-!
Disposable verification obligations for `zerocopy_slice::util_max`.

This file is generated on every `cargo anneal` run. Do not edit it by
hand; declare an `ItemSpec` instance in `Specs.ZerocopySlice.util_max` or discharge the proof in `Proof.ZerocopySlice.util_max`.

Linked specification companion items:
- Inline requires: `zerocopy_slice::__anodized_fn_requires_util_max` (`Mir.ZerocopySlice.__anodized_fn_requires_util_max`)
- Inline ensures: `zerocopy_slice::__anodized_fn_ensures_util_max` (`Mir.ZerocopySlice.__anodized_fn_ensures_util_max`)
-/

/-- Default precondition lowered automatically from Rust `#[spec]` /
    `#[external_spec]` `requires` companions for `zerocopy_slice::util_max` (or `True`
    when no companion contract is present). -/
def defaultPre (args : List Value) : Prop :=
  And (Trusted.Sequence.length args = Mir.ZerocopySlice.util_max.body.argCount) (MirBoolPredicateHolds Mir.ZerocopySlice.__anodized_fn_requires_util_max.body args)

/-- Default postcondition lowered automatically from Rust `#[spec]` /
    `#[external_spec]` `ensures` companions for `zerocopy_slice::util_max` (or `True`
    when no companion contract is present). -/
def defaultPost (args : List Value) (ret : Value) : Prop :=
  MirBoolPredicateHolds Mir.ZerocopySlice.__anodized_fn_ensures_util_max.body (List.append args (List.cons ret List.nil))

/-- Trusted `requires` predicate lowered from companion `#[spec]` and
    `#[external_spec]` items for `zerocopy_slice::util_max`. -/
abbrev rustRequires : List Value → Prop := defaultPre

/-- Trusted `ensures` relation lowered from companion `#[spec]` and
    `#[external_spec]` items for `zerocopy_slice::util_max`. -/
abbrev rustEnsures : List Value → Value → Prop := defaultPost

/-- Low-priority default `ItemSpec` instance for `zerocopy_slice::util_max`.
    An authored normal-priority `ItemSpec` instance in `Specs.ZerocopySlice.util_max`
    automatically overrides this instance via Lean typeclass
    synthesis. -/
instance (priority := low) defaultSpec : ItemSpec "zerocopy_slice::util_max" where
  pre := defaultPre
  post := defaultPost

/-- Low-priority default `StatefulItemSpec` instance for `zerocopy_slice::util_max`. -/
instance (priority := low) defaultStatefulSpec :
    StatefulItemSpec "zerocopy_slice::util_max" where
  pre := fun args _ => defaultPre args
  post := fun args _ ret _ => defaultPost args ret

/-- Refine `defaultSpec` by conjoining additional Lean `extraPre` and
    `extraPost` clauses while preserving `rustRequires` and `rustEnsures`. -/
def refineSpec
    (extraPre : List Value → Prop)
    (extraPost : List Value → Value → Prop) :
    ItemSpec "zerocopy_slice::util_max" where
  pre args := And (rustRequires args) (extraPre args)
  post args ret := And (rustEnsures args ret) (extraPost args ret)

/-- Refine `defaultStatefulSpec` by conjoining additional stateful Lean
    `extraPre` and `extraPost` clauses while preserving `rustRequires` and
    `rustEnsures`. -/
def refineStatefulSpec
    (extraPre : List Value → Memory → Prop)
    (extraPost : List Value → Memory → Value → Memory → Prop) :
    StatefulItemSpec "zerocopy_slice::util_max" where
  pre args m := And (rustRequires args) (extraPre args m)
  post args m0 ret m1 := And (rustEnsures args ret) (extraPost args m0 ret m1)

/-- Refine `defaultSpec` for a safe function whose execution is defined
    (`MirNoUB`, `.defined` of `Partial`) across `safetyDomain` while guarding
    `rustEnsures` and `extraPost` behind `rustRequires` and `extraPre` inside
    `.val` (`#125`). -/
def refineSafeSpec
    (safetyDomain : List Value → Prop)
    (extraPre : List Value → Prop)
    (extraPost : List Value → Value → Prop) :
    ItemSpec "zerocopy_slice::util_max" where
  pre args := safetyDomain args
  post args ret :=
    And (rustRequires args) (extraPre args) →
      And (rustEnsures args ret) (extraPost args ret)

/-- Refine `defaultStatefulSpec` for a safe stateful function whose execution
    is defined (`MirNoUB`, `.defined` of `Partial`) across `safetyDomain`
    while guarding `rustEnsures` and `extraPost` behind `rustRequires` and
    `extraPre` inside `.val` (`#125`). -/
def refineSafeStatefulSpec
    (safetyDomain : List Value → Memory → Prop)
    (extraPre : List Value → Memory → Prop)
    (extraPost : List Value → Memory → Value → Memory → Prop) :
    StatefulItemSpec "zerocopy_slice::util_max" where
  pre args m := safetyDomain args m
  post args m0 ret m1 :=
    And (rustRequires args) (extraPre args m0) →
      And (rustEnsures args ret) (extraPost args m0 ret m1)

/-- Canonical verification claim for `zerocopy_slice::util_max` synthesized from
    `ItemSpec "zerocopy_slice::util_max"`. -/
def claim [spec : ItemSpec "zerocopy_slice::util_max"] : Claim :=
  { body := Mir.ZerocopySlice.util_max.body, pre := spec.pre, post := spec.post }

/-- Canonical stateful verification claim for `zerocopy_slice::util_max` synthesized
    from `StatefulItemSpec "zerocopy_slice::util_max"`. -/
def statefulClaim [spec : StatefulItemSpec "zerocopy_slice::util_max"] : StatefulClaim :=
  { body := Mir.ZerocopySlice.util_max.body, pre := spec.pre, post := spec.post }

end Obligations.ZerocopySlice.util_max
