prelude
import Init.Core
import Trusted
import Anneal
import Mir.ZerocopySlice.BigEndianU16_new
import Specs.ZerocopySlice.BigEndianU16_new
import Obligations.ZerocopySlice.BigEndianU16_new

namespace Proof.ZerocopySlice.BigEndianU16_new

open Trusted.VerificationResult
open Anneal.BitValidity
open Anneal.Rustc.V2026_06_01.Lower
open Anneal.Tactics

theorem swap_witness : swapBytes16 0x1234 = 0x3412 :=
  swapBytes16_0x1234

set_option maxHeartbeats 2000000 in
theorem verified :
    VerificationResult Obligations.ZerocopySlice.BigEndianU16_new.claim :=
  let _ := swap_witness
  let _ := @encodeLittleEndianBytes_swapBytes16_0x1234
  let _ := @transmute_u16_to_byteOrderU16_sound
  dischargeStatelessEq
    Obligations.ZerocopySlice.BigEndianU16_new.claim
    32
    (List.cons (.scalar { bits := 0x1234, sizeBytes := 2 }) List.nil)
    (fun _ h => h)
    rfl

end Proof.ZerocopySlice.BigEndianU16_new
