prelude
import Init.Core
import Trusted
import Mir.ZerocopySlice.Unalign_u8_deref
import Obligations.ZerocopySlice.Unalign_u8_deref
import Specs.ZerocopySlice.Unalign_u8_deref
import Proof.ZerocopySlice.Unalign_u8_deref

namespace Verify.ZerocopySlice.Unalign_u8_deref

open Trusted.VerificationResult

/-!
Disposable verification assembly root for `ZerocopySlice.Unalign_u8_deref`.

This module is generated on every `cargo anneal` run and assembles the
active `ItemSpec` and proof witness at the audited
`VerificationResult` theorem boundary.
-/

/-- Kernel-checked verification certificate for `ZerocopySlice.Unalign_u8_deref`. -/
theorem verified : VerificationResult Obligations.ZerocopySlice.Unalign_u8_deref.claim :=
  Proof.ZerocopySlice.Unalign_u8_deref.verified

end Verify.ZerocopySlice.Unalign_u8_deref
