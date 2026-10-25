prelude
import Init.Core
import Trusted
import Anneal
import Mir.ZerocopySlice.Ptr_a_u8_slice_len
import Specs.ZerocopySlice.Ptr_a_u8_slice_len
import Obligations.ZerocopySlice.Ptr_a_u8_slice_len

namespace Proof.ZerocopySlice.Ptr_a_u8_slice_len

open Trusted.Rustc.V2026_06_01.MirSemantics
open Trusted.VerificationResult
open Anneal.UnsafeObligations
open Anneal.Rustc.V2026_06_01.Lower
open Anneal.Tactics
open Specs.ZerocopySlice.Ptr_a_u8_slice_len

theorem slice_split_disjoint_witness :
    ValidSliceSplit { allocId := 1, offset := 0, tag := 1 } 2 4 2 2 ∧
    NonOverlappingPtrs
      { allocId := 1, offset := 0, tag := 1 }
      (sliceSplitRightPtr { allocId := 1, offset := 0, tag := 1 } 2 2)
      (Nat.mul 2 2) :=
  canary_split_at_u16_disjoint

set_option maxHeartbeats 2000000 in
theorem verified :
    StatefulVerificationResult Obligations.ZerocopySlice.Ptr_a_u8_slice_len.statefulClaim :=
  let _ := slice_split_disjoint_witness
  verifyStatefulByWPSymbolic
    Obligations.ZerocopySlice.Ptr_a_u8_slice_len.statefulClaim
    16
    (fun len : Nat => slicePtrArg len :: .nil)
    (fun _ : Nat => { emptyMemory with calleeContracts := sliceLenContract :: .nil })
    (fun _ : Nat => True)
    (fun _ _ ⟨len, h_args, h_mem⟩ => ⟨len, h_args, h_mem, True.intro⟩)
    (fun len _ => ⟨len, rfl, rfl⟩)

end Proof.ZerocopySlice.Ptr_a_u8_slice_len
