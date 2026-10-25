prelude
import Init.Core
import Trusted
import Mir.ZerocopySlice.__anneal_unsafe_impl_Unaligned_for_BigEndianU16
import Obligations.ZerocopySlice.__anneal_unsafe_impl_Unaligned_for_BigEndianU16
import Proof.ZerocopySlice.__anneal_unsafe_impl_Unaligned_for_BigEndianU16

namespace Verify.ZerocopySlice.__anneal_unsafe_impl_Unaligned_for_BigEndianU16

open Trusted.VerificationResult

/-!
Disposable verification assembly root for `ZerocopySlice.__anneal_unsafe_impl_Unaligned_for_BigEndianU16`.

This module is generated on every `cargo anneal` run and assembles the
active `ItemSpec` and proof witness at the audited
`VerificationResult` theorem boundary.
-/

/-- Kernel-checked verification certificate for `ZerocopySlice.__anneal_unsafe_impl_Unaligned_for_BigEndianU16`. -/
theorem verified : VerificationResult Obligations.ZerocopySlice.__anneal_unsafe_impl_Unaligned_for_BigEndianU16.claim :=
  Proof.ZerocopySlice.__anneal_unsafe_impl_Unaligned_for_BigEndianU16.verified

end Verify.ZerocopySlice.__anneal_unsafe_impl_Unaligned_for_BigEndianU16
