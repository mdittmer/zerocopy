prelude
import Init.Core
import Trusted
import Mir.ZerocopySlice.Ptr_a_PacketHeader_as_ref

namespace Obligations.ZerocopySlice.Ptr_a_PacketHeader_as_ref

set_option linter.unusedVariables false

open Trusted.MirSemantics
open Trusted.VerificationResult

/-!
Disposable verification obligations for `zerocopy_slice::Ptr::<'a, PacketHeader>::as_ref`.

This file is generated on every `cargo anneal` run. Do not edit it by
hand; declare an `ItemSpec` instance in `Specs.ZerocopySlice.Ptr_a_PacketHeader_as_ref` or discharge the proof in `Proof.ZerocopySlice.Ptr_a_PacketHeader_as_ref`.
-/

/-- Default precondition lowered automatically from Rust `#[spec]` /
    `#[external_spec]` `requires` companions for `zerocopy_slice::Ptr::<'a, PacketHeader>::as_ref` (or `True`
    when no companion contract is present). -/
def defaultPre (args : List Value) : Prop :=
  True

/-- Default postcondition lowered automatically from Rust `#[spec]` /
    `#[external_spec]` `ensures` companions for `zerocopy_slice::Ptr::<'a, PacketHeader>::as_ref` (or `True`
    when no companion contract is present). -/
def defaultPost (args : List Value) (ret : Value) : Prop :=
  True

/-- Trusted `requires` predicate lowered from companion `#[spec]` and
    `#[external_spec]` items for `zerocopy_slice::Ptr::<'a, PacketHeader>::as_ref`. -/
abbrev rustRequires : List Value → Prop := defaultPre

/-- Trusted `ensures` relation lowered from companion `#[spec]` and
    `#[external_spec]` items for `zerocopy_slice::Ptr::<'a, PacketHeader>::as_ref`. -/
abbrev rustEnsures : List Value → Value → Prop := defaultPost

/-- Low-priority default `ItemSpec` instance for `zerocopy_slice::Ptr::<'a, PacketHeader>::as_ref`.
    An authored normal-priority `ItemSpec` instance in `Specs.ZerocopySlice.Ptr_a_PacketHeader_as_ref`
    automatically overrides this instance via Lean typeclass
    synthesis. -/
instance (priority := low) defaultSpec : ItemSpec "zerocopy_slice::Ptr::<'a, PacketHeader>::as_ref" where
  pre := defaultPre
  post := defaultPost

/-- Low-priority default `StatefulItemSpec` instance for `zerocopy_slice::Ptr::<'a, PacketHeader>::as_ref`. -/
instance (priority := low) defaultStatefulSpec :
    StatefulItemSpec "zerocopy_slice::Ptr::<'a, PacketHeader>::as_ref" where
  pre := fun args _ => defaultPre args
  post := fun args _ ret _ => defaultPost args ret

/-- Refine `defaultSpec` by conjoining additional Lean `extraPre` and
    `extraPost` clauses while preserving `rustRequires` and `rustEnsures`. -/
def refineSpec
    (extraPre : List Value → Prop)
    (extraPost : List Value → Value → Prop) :
    ItemSpec "zerocopy_slice::Ptr::<'a, PacketHeader>::as_ref" where
  pre args := And (rustRequires args) (extraPre args)
  post args ret := And (rustEnsures args ret) (extraPost args ret)

/-- Refine `defaultStatefulSpec` by conjoining additional stateful Lean
    `extraPre` and `extraPost` clauses while preserving `rustRequires` and
    `rustEnsures`. -/
def refineStatefulSpec
    (extraPre : List Value → Memory → Prop)
    (extraPost : List Value → Memory → Value → Memory → Prop) :
    StatefulItemSpec "zerocopy_slice::Ptr::<'a, PacketHeader>::as_ref" where
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
    ItemSpec "zerocopy_slice::Ptr::<'a, PacketHeader>::as_ref" where
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
    StatefulItemSpec "zerocopy_slice::Ptr::<'a, PacketHeader>::as_ref" where
  pre args m := safetyDomain args m
  post args m0 ret m1 :=
    And (rustRequires args) (extraPre args m0) →
      And (rustEnsures args ret) (extraPost args m0 ret m1)

/-- Canonical verification claim for `zerocopy_slice::Ptr::<'a, PacketHeader>::as_ref` synthesized from
    `ItemSpec "zerocopy_slice::Ptr::<'a, PacketHeader>::as_ref"`. -/
def claim [spec : ItemSpec "zerocopy_slice::Ptr::<'a, PacketHeader>::as_ref"] : Claim :=
  { body := Mir.ZerocopySlice.Ptr_a_PacketHeader_as_ref.body, pre := spec.pre, post := spec.post }

/-- Canonical stateful verification claim for `zerocopy_slice::Ptr::<'a, PacketHeader>::as_ref` synthesized
    from `StatefulItemSpec "zerocopy_slice::Ptr::<'a, PacketHeader>::as_ref"`. -/
def statefulClaim [spec : StatefulItemSpec "zerocopy_slice::Ptr::<'a, PacketHeader>::as_ref"] : StatefulClaim :=
  { body := Mir.ZerocopySlice.Ptr_a_PacketHeader_as_ref.body, pre := spec.pre, post := spec.post }

end Obligations.ZerocopySlice.Ptr_a_PacketHeader_as_ref
