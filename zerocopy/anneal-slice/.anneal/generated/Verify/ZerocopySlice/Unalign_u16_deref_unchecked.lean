prelude
import Init.Core
import Trusted
import Mir.ZerocopySlice.Unalign_u16_deref_unchecked
import Obligations.ZerocopySlice.Unalign_u16_deref_unchecked
import Specs.ZerocopySlice.Unalign_u16_deref_unchecked
import Proof.ZerocopySlice.Unalign_u16_deref_unchecked

namespace Verify.ZerocopySlice.Unalign_u16_deref_unchecked

open Trusted.VerificationResult

/-!
Disposable verification assembly root for `ZerocopySlice.Unalign_u16_deref_unchecked`.

This module is generated on every `cargo anneal` run and assembles the
active `ItemSpec` and proof witness at the audited
`VerificationResult` theorem boundary.
-/

/-- Kernel-checked stateful verification certificate for `ZerocopySlice.Unalign_u16_deref_unchecked`. -/
theorem verified : StatefulVerificationResult Obligations.ZerocopySlice.Unalign_u16_deref_unchecked.statefulClaim :=
  Proof.ZerocopySlice.Unalign_u16_deref_unchecked.verified

end Verify.ZerocopySlice.Unalign_u16_deref_unchecked
