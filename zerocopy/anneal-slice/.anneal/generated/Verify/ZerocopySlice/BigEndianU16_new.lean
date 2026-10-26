prelude
import Init.Core
import Trusted
import Mir.ZerocopySlice.BigEndianU16_new
import Obligations.ZerocopySlice.BigEndianU16_new
import Specs.ZerocopySlice.BigEndianU16_new
import Proof.ZerocopySlice.BigEndianU16_new

namespace Verify.ZerocopySlice.BigEndianU16_new

open Trusted.VerificationResult

/-!
Disposable verification assembly root for `ZerocopySlice.BigEndianU16_new`.

This module is generated on every `cargo anneal` run and assembles the
active `ItemSpec` and proof witness at the audited
`VerificationResult` theorem boundary.
-/

/-- Kernel-checked verification certificate for `ZerocopySlice.BigEndianU16_new`. -/
theorem verified : VerificationResult Obligations.ZerocopySlice.BigEndianU16_new.claim :=
  Proof.ZerocopySlice.BigEndianU16_new.verified

end Verify.ZerocopySlice.BigEndianU16_new
