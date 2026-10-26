prelude
import Init.Core
import Trusted
import Anneal
import Mir.ZerocopySlice.Unalign_u16_set
import Specs.ZerocopySlice.Unalign_u16_set
import Obligations.ZerocopySlice.Unalign_u16_set

namespace Proof.ZerocopySlice.Unalign_u16_set

open Trusted.MirSemantics
open Trusted.VerificationResult
open Anneal.SepLogic
open Anneal.Polymorphism
open Anneal.Rustc.V2026_06_01.Lower

theorem parametric_set_u16_witness :
    VerificationResult (parametricUnalignSetClaim { size := 2, align := 2 }) :=
  unalign_set_u16_inst

theorem parametric_set_byteorder_u16_witness :
    VerificationResult (parametricUnalignSetClaim byteOrderU16LayoutDesc) :=
  unalign_set_byteorder_u16_inst

set_option maxHeartbeats 2000000 in
theorem verified :
    StatefulVerificationResult Obligations.ZerocopySlice.Unalign_u16_set.statefulClaim :=
  let _ := parametric_set_u16_witness
  let _ := parametric_set_byteorder_u16_witness
  let _ := @unalign_set_mir_verified
  verifyStatefulByWP Obligations.ZerocopySlice.Unalign_u16_set.statefulClaim 32 (by
    intro args mem ⟨h_args, h_mem⟩
    subst h_args
    subst h_mem
    exact ⟨rfl, rfl⟩)

end Proof.ZerocopySlice.Unalign_u16_set
