prelude
import Init.Core
import Trusted
import Mir.ZerocopySlice.round_down_to_next_multiple_of_alignment
import Obligations.ZerocopySlice.round_down_to_next_multiple_of_alignment
import Specs.ZerocopySlice.round_down_to_next_multiple_of_alignment
import Proof.ZerocopySlice.round_down_to_next_multiple_of_alignment

namespace Verify.ZerocopySlice.round_down_to_next_multiple_of_alignment

open Trusted.VerificationResult

/-!
Disposable verification assembly root for `ZerocopySlice.round_down_to_next_multiple_of_alignment`.

This module is generated on every `cargo anneal` run and assembles the
active `ItemSpec` and proof witness at the audited
`VerificationResult` theorem boundary.
-/

/-- Kernel-checked verification certificate for `ZerocopySlice.round_down_to_next_multiple_of_alignment`. -/
theorem verified : VerificationResult Obligations.ZerocopySlice.round_down_to_next_multiple_of_alignment.claim :=
  Proof.ZerocopySlice.round_down_to_next_multiple_of_alignment.verified

end Verify.ZerocopySlice.round_down_to_next_multiple_of_alignment
