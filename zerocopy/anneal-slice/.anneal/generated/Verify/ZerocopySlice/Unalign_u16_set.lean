prelude
import Init.Core
import Trusted
import Mir.ZerocopySlice.Unalign_u16_set
import Obligations.ZerocopySlice.Unalign_u16_set
import Specs.ZerocopySlice.Unalign_u16_set
import Proof.ZerocopySlice.Unalign_u16_set

namespace Verify.ZerocopySlice.Unalign_u16_set

open Trusted.VerificationResult

/-!
Disposable verification assembly root for `ZerocopySlice.Unalign_u16_set`.

This module is generated on every `cargo anneal` run and assembles the
active `ItemSpec` and proof witness at the audited
`VerificationResult` theorem boundary.
-/

/-- Kernel-checked stateful verification certificate for `ZerocopySlice.Unalign_u16_set`. -/
theorem verified : StatefulVerificationResult Obligations.ZerocopySlice.Unalign_u16_set.statefulClaim :=
  Proof.ZerocopySlice.Unalign_u16_set.verified

end Verify.ZerocopySlice.Unalign_u16_set
