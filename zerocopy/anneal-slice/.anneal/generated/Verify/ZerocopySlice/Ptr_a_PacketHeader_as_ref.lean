prelude
import Init.Core
import Trusted
import Mir.ZerocopySlice.Ptr_a_PacketHeader_as_ref
import Obligations.ZerocopySlice.Ptr_a_PacketHeader_as_ref
import Specs.ZerocopySlice.Ptr_a_PacketHeader_as_ref
import Proof.ZerocopySlice.Ptr_a_PacketHeader_as_ref

namespace Verify.ZerocopySlice.Ptr_a_PacketHeader_as_ref

open Trusted.VerificationResult

/-!
Disposable verification assembly root for `ZerocopySlice.Ptr_a_PacketHeader_as_ref`.

This module is generated on every `cargo anneal` run and assembles the
active `ItemSpec` and proof witness at the audited
`VerificationResult` theorem boundary.
-/

/-- Kernel-checked stateful verification certificate for `ZerocopySlice.Ptr_a_PacketHeader_as_ref`. -/
theorem verified : StatefulVerificationResult Obligations.ZerocopySlice.Ptr_a_PacketHeader_as_ref.statefulClaim :=
  Proof.ZerocopySlice.Ptr_a_PacketHeader_as_ref.verified

end Verify.ZerocopySlice.Ptr_a_PacketHeader_as_ref
