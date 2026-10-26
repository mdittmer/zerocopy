prelude
import Init.Core
import Trusted
import Anneal
import Mir.ZerocopySlice.util_max
import Obligations.ZerocopySlice.util_max

namespace Proof.ZerocopySlice.util_max

open Trusted.MirSemantics
open Trusted.VerificationResult
open Anneal.WP
open Anneal.Tactics
open Anneal.Rustc.V2026_06_01.Lower

theorem well_typed_args_witness :
    VerificationResult canaryWellTypedFirstU64Claim :=
  canary_well_typed_scalar_arg2_verified

set_option maxHeartbeats 2000000 in
theorem verified : VerificationResult Obligations.ZerocopySlice.util_max.claim :=
  let _ := well_typed_args_witness
  dischargeRustEq2U64
    Obligations.ZerocopySlice.util_max.claim
    16
    64
    Mir.ZerocopySlice.__anodized_fn_requires_util_max.body
    Mir.ZerocopySlice.__anodized_fn_ensures_util_max.body
    4
    8

end Proof.ZerocopySlice.util_max
