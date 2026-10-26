prelude
import Init.Core
import Trusted
import Anneal
import Mir.ZerocopySlice.validate_aligned_to
import Specs.ZerocopySlice.validate_aligned_to
import Obligations.ZerocopySlice.validate_aligned_to

namespace Proof.ZerocopySlice.validate_aligned_to

open Trusted.Rustc.V2026_06_01.MirSemantics
open Trusted.VerificationResult
open Anneal.UnsafeObligations
open Anneal.Rustc.V2026_06_01.Lower
open Anneal.Tactics

theorem aligned_ptr_24_8 :
    AlignedPtr { allocId := 1, offset := 24, tag := 1 } 8 :=
  ⟨by decide, rfl⟩

def unalignedAddrRejectedClaim : Claim where
  body := Mir.ZerocopySlice.validate_aligned_to.body
  pre := fun args =>
    args =
      List.cons (.scalar { bits := 23, sizeBytes := 8 })
        (List.cons (.scalar { bits := 8, sizeBytes := 8 }) List.nil)
  post := fun _ ret => ret = .scalar { bits := 0, sizeBytes := 1 }

set_option maxHeartbeats 2000000 in
theorem unaligned_rejected : VerificationResult unalignedAddrRejectedClaim :=
  dischargeStatelessEq
    unalignedAddrRejectedClaim
    32
    (List.cons (.scalar { bits := 23, sizeBytes := 8 })
      (List.cons (.scalar { bits := 8, sizeBytes := 8 }) List.nil))
    (fun _ h => h)
    rfl

set_option maxHeartbeats 2000000 in
theorem verified :
    VerificationResult Obligations.ZerocopySlice.validate_aligned_to.claim :=
  let _ := aligned_ptr_24_8
  let _ := unaligned_rejected
  dischargeStatelessEq
    Obligations.ZerocopySlice.validate_aligned_to.claim
    32
    (List.cons (.scalar { bits := 24, sizeBytes := 8 })
      (List.cons (.scalar { bits := 8, sizeBytes := 8 }) List.nil))
    (fun _ h => h)
    rfl

end Proof.ZerocopySlice.validate_aligned_to
