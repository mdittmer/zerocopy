prelude
import Init.Core
import Trusted
import Mir.ZerocopySlice.Ptr_a_u8_slice_len
import Obligations.ZerocopySlice.Ptr_a_u8_slice_len
import Specs.ZerocopySlice.Ptr_a_u8_slice_len
import Proof.ZerocopySlice.Ptr_a_u8_slice_len

namespace Verify.ZerocopySlice.Ptr_a_u8_slice_len

open Trusted.VerificationResult

/-!
Disposable verification assembly root for `ZerocopySlice.Ptr_a_u8_slice_len`.

This module is generated on every `cargo anneal` run and assembles the
active `ItemSpec` and proof witness at the audited
`VerificationResult` theorem boundary.
-/

/-- Kernel-checked stateful verification certificate for `ZerocopySlice.Ptr_a_u8_slice_len`. -/
theorem verified : StatefulVerificationResult Obligations.ZerocopySlice.Ptr_a_u8_slice_len.statefulClaim :=
  Proof.ZerocopySlice.Ptr_a_u8_slice_len.verified

end Verify.ZerocopySlice.Ptr_a_u8_slice_len
