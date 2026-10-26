prelude
import Init.Core
import Trusted
import Mir.ZerocopySlice.util_max
import Obligations.ZerocopySlice.util_max
import Proof.ZerocopySlice.util_max

namespace Verify.ZerocopySlice.util_max

open Trusted.VerificationResult

/-!
Disposable verification assembly root for `ZerocopySlice.util_max`.

This module is generated on every `cargo anneal` run and assembles the
active `ItemSpec` and proof witness at the audited
`VerificationResult` theorem boundary.
-/

/-- Kernel-checked verification certificate for `ZerocopySlice.util_max`. -/
theorem verified : VerificationResult Obligations.ZerocopySlice.util_max.claim :=
  Proof.ZerocopySlice.util_max.verified

end Verify.ZerocopySlice.util_max
