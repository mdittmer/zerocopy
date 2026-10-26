prelude
import Init.Core
import Trusted
import Mir.ZerocopySlice.Unalign_u16_new
import Obligations.ZerocopySlice.Unalign_u16_new
import Specs.ZerocopySlice.Unalign_u16_new
import Proof.ZerocopySlice.Unalign_u16_new

namespace Verify.ZerocopySlice.Unalign_u16_new

open Trusted.VerificationResult

/-!
Disposable verification assembly root for `ZerocopySlice.Unalign_u16_new`.

This module is generated on every `cargo anneal` run and assembles the
active `ItemSpec` and proof witness at the audited
`VerificationResult` theorem boundary.
-/

/-- Kernel-checked verification certificate for `ZerocopySlice.Unalign_u16_new`. -/
theorem verified : VerificationResult Obligations.ZerocopySlice.Unalign_u16_new.claim :=
  Proof.ZerocopySlice.Unalign_u16_new.verified

end Verify.ZerocopySlice.Unalign_u16_new
