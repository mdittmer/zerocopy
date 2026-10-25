prelude
import Init.Core
import Trusted
import Anneal
import Mir.ZerocopySlice.BigEndianU16_get
import Specs.ZerocopySlice.BigEndianU16_get
import Obligations.ZerocopySlice.BigEndianU16_get

namespace Proof.ZerocopySlice.BigEndianU16_get

open Trusted.VerificationResult
open Anneal.BitValidity
open Anneal.Rustc.V2026_06_01.Lower
open Anneal.Tactics

theorem swap_involutive_witness :
    swapBytes16 (swapBytes16 0x1234) = 0x1234 :=
  swapBytes16_involutive_0x1234

set_option maxHeartbeats 2000000 in
theorem verified :
    VerificationResult Obligations.ZerocopySlice.BigEndianU16_get.claim :=
  let _ := swap_involutive_witness
  let _ := @transmute_byteOrderU16_to_u16_sound
  dischargeStatelessEq
    Obligations.ZerocopySlice.BigEndianU16_get.claim
    32
    (List.cons
      (.aggregate 0
        (List.cons
          (.aggregate 0
            (List.cons (.scalar { bits := 0x12, sizeBytes := 1 })
              (List.cons (.scalar { bits := 0x34, sizeBytes := 1 }) List.nil)))
          List.nil))
      List.nil)
    (fun _ h => h)
    rfl

end Proof.ZerocopySlice.BigEndianU16_get
