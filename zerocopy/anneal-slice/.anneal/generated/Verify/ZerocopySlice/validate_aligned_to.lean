prelude
import Init.Core
import Trusted
import Mir.ZerocopySlice.validate_aligned_to
import Obligations.ZerocopySlice.validate_aligned_to
import Specs.ZerocopySlice.validate_aligned_to
import Proof.ZerocopySlice.validate_aligned_to

namespace Verify.ZerocopySlice.validate_aligned_to

open Trusted.VerificationResult

/-!
Disposable verification assembly root for `ZerocopySlice.validate_aligned_to`.

This module is generated on every `cargo anneal` run and assembles the
active `ItemSpec` and proof witness at the audited
`VerificationResult` theorem boundary.
-/

/-- Kernel-checked verification certificate for `ZerocopySlice.validate_aligned_to`. -/
theorem verified : VerificationResult Obligations.ZerocopySlice.validate_aligned_to.claim :=
  Proof.ZerocopySlice.validate_aligned_to.verified

end Verify.ZerocopySlice.validate_aligned_to
