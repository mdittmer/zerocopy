prelude
import Init.Core
import Trusted
import Mir.ZerocopySlice.padding_needed_for
import Obligations.ZerocopySlice.padding_needed_for
import Specs.ZerocopySlice.padding_needed_for
import Proof.ZerocopySlice.padding_needed_for

namespace Verify.ZerocopySlice.padding_needed_for

open Trusted.VerificationResult

/-!
Disposable verification assembly root for `ZerocopySlice.padding_needed_for`.

This module is generated on every `cargo anneal` run and assembles the
active `ItemSpec` and proof witness at the audited
`VerificationResult` theorem boundary.
-/

/-- Kernel-checked stateful verification certificate for `ZerocopySlice.padding_needed_for`. -/
theorem verified : StatefulVerificationResult Obligations.ZerocopySlice.padding_needed_for.statefulClaim :=
  Proof.ZerocopySlice.padding_needed_for.verified

end Verify.ZerocopySlice.padding_needed_for
