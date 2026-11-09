prelude
import Init.Core
import Trusted
import Mir.ZerocopySlice.Unalign_u16_get
import Obligations.ZerocopySlice.Unalign_u16_get
import Specs.ZerocopySlice.Unalign_u16_get
import Proof.ZerocopySlice.Unalign_u16_get

namespace Verify.ZerocopySlice.Unalign_u16_get

open Trusted.VerificationResult

/-!
Disposable verification assembly root for `ZerocopySlice.Unalign_u16_get`.

This module is generated on every `cargo anneal` run and assembles the
active `ItemSpec` and proof witness at the audited
`VerificationResult` theorem boundary.
-/

/-- Kernel-checked stateful verification certificate for `ZerocopySlice.Unalign_u16_get`. -/
theorem verified : StatefulVerificationResult Obligations.ZerocopySlice.Unalign_u16_get.statefulClaim :=
  Proof.ZerocopySlice.Unalign_u16_get.verified

end Verify.ZerocopySlice.Unalign_u16_get
