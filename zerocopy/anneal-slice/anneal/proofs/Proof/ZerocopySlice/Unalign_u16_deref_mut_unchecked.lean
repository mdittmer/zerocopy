prelude
import Init.Core
import Trusted
import Anneal
import Mir.ZerocopySlice.Unalign_u16_deref_mut_unchecked
import Specs.ZerocopySlice.Unalign_u16_deref_mut_unchecked
import Obligations.ZerocopySlice.Unalign_u16_deref_mut_unchecked

namespace Proof.ZerocopySlice.Unalign_u16_deref_mut_unchecked

open Trusted.MirSemantics
open Trusted.VerificationResult
open Anneal.UnsafeObligations
open Anneal.SepLogic
open Anneal.Rustc.V2026_06_01.Lower

theorem deref_mut_unchecked_aligned :
    AlignedPtr { allocId := 2, offset := 8, tag := 2 } 2 :=
  ⟨by decide, rfl⟩

set_option maxHeartbeats 2000000 in
theorem verified :
    StatefulVerificationResult
      Obligations.ZerocopySlice.Unalign_u16_deref_mut_unchecked.statefulClaim :=
  let _ := deref_mut_unchecked_aligned
  let _ := @unalign_deref_mut_unchecked_sound
  verifyStatefulByWP
    Obligations.ZerocopySlice.Unalign_u16_deref_mut_unchecked.statefulClaim
    32
    (by
      intro args mem ⟨h_args, h_mem⟩
      subst h_args
      subst h_mem
      exact ⟨rfl, rfl, rfl⟩)

end Proof.ZerocopySlice.Unalign_u16_deref_mut_unchecked
