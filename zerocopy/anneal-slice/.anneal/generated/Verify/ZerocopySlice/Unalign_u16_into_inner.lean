prelude
import Init.Core
import Trusted
import Mir.ZerocopySlice.Unalign_u16_into_inner
import Obligations.ZerocopySlice.Unalign_u16_into_inner
import Specs.ZerocopySlice.Unalign_u16_into_inner
import Proof.ZerocopySlice.Unalign_u16_into_inner

namespace Verify.ZerocopySlice.Unalign_u16_into_inner

open Trusted.VerificationResult

/-!
Disposable verification assembly root for `ZerocopySlice.Unalign_u16_into_inner`.

This module is generated on every `cargo anneal` run and assembles the
active `ItemSpec` and proof witness at the audited
`VerificationResult` theorem boundary.
-/

/-- Kernel-checked verification certificate for `ZerocopySlice.Unalign_u16_into_inner`. -/
theorem verified : VerificationResult Obligations.ZerocopySlice.Unalign_u16_into_inner.claim :=
  Proof.ZerocopySlice.Unalign_u16_into_inner.verified

end Verify.ZerocopySlice.Unalign_u16_into_inner
