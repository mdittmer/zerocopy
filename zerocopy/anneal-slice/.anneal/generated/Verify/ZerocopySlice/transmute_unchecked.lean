prelude
import Init.Core
import Trusted
import Mir.ZerocopySlice.transmute_unchecked
import Obligations.ZerocopySlice.transmute_unchecked
import Specs.ZerocopySlice.transmute_unchecked
import Proof.ZerocopySlice.transmute_unchecked

namespace Verify.ZerocopySlice.transmute_unchecked

open Trusted.VerificationResult

/-!
Disposable verification assembly root for `ZerocopySlice.transmute_unchecked`.

This module is generated on every `cargo anneal` run and assembles the
active `ItemSpec` and proof witness at the audited
`VerificationResult` theorem boundary.
-/

/-- Kernel-checked verification certificate for `ZerocopySlice.transmute_unchecked`. -/
theorem verified : VerificationResult Obligations.ZerocopySlice.transmute_unchecked.claim :=
  Proof.ZerocopySlice.transmute_unchecked.verified

end Verify.ZerocopySlice.transmute_unchecked
