prelude
import Init.Core
import Trusted
import Mir.ZerocopySlice.__anneal_unsafe_impl_FromBytes_for_PacketHeader
import Obligations.ZerocopySlice.__anneal_unsafe_impl_FromBytes_for_PacketHeader
import Proof.ZerocopySlice.__anneal_unsafe_impl_FromBytes_for_PacketHeader

namespace Verify.ZerocopySlice.__anneal_unsafe_impl_FromBytes_for_PacketHeader

open Trusted.VerificationResult

/-!
Disposable verification assembly root for `ZerocopySlice.__anneal_unsafe_impl_FromBytes_for_PacketHeader`.

This module is generated on every `cargo anneal` run and assembles the
active `ItemSpec` and proof witness at the audited
`VerificationResult` theorem boundary.
-/

/-- Kernel-checked verification certificate for `ZerocopySlice.__anneal_unsafe_impl_FromBytes_for_PacketHeader`. -/
theorem verified : VerificationResult Obligations.ZerocopySlice.__anneal_unsafe_impl_FromBytes_for_PacketHeader.claim :=
  Proof.ZerocopySlice.__anneal_unsafe_impl_FromBytes_for_PacketHeader.verified

end Verify.ZerocopySlice.__anneal_unsafe_impl_FromBytes_for_PacketHeader
