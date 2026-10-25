prelude
import Init.Core
import Trusted
import Mir.ZerocopySlice.BigEndianU16_get
import Obligations.ZerocopySlice.BigEndianU16_get
import Specs.ZerocopySlice.BigEndianU16_get
import Proof.ZerocopySlice.BigEndianU16_get

namespace Verify.ZerocopySlice.BigEndianU16_get

open Trusted.VerificationResult

/-!
Disposable verification assembly root for `ZerocopySlice.BigEndianU16_get`.

This module is generated on every `cargo anneal` run and assembles the
active `ItemSpec` and proof witness at the audited
`VerificationResult` theorem boundary.
-/

/-- Kernel-checked verification certificate for `ZerocopySlice.BigEndianU16_get`. -/
theorem verified : VerificationResult Obligations.ZerocopySlice.BigEndianU16_get.claim :=
  Proof.ZerocopySlice.BigEndianU16_get.verified

end Verify.ZerocopySlice.BigEndianU16_get
