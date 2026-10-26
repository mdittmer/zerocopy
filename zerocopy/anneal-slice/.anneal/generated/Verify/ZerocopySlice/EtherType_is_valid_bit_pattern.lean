prelude
import Init.Core
import Trusted
import Mir.ZerocopySlice.EtherType_is_valid_bit_pattern
import Obligations.ZerocopySlice.EtherType_is_valid_bit_pattern
import Specs.ZerocopySlice.EtherType_is_valid_bit_pattern
import Proof.ZerocopySlice.EtherType_is_valid_bit_pattern

namespace Verify.ZerocopySlice.EtherType_is_valid_bit_pattern

open Trusted.VerificationResult

/-!
Disposable verification assembly root for `ZerocopySlice.EtherType_is_valid_bit_pattern`.

This module is generated on every `cargo anneal` run and assembles the
active `ItemSpec` and proof witness at the audited
`VerificationResult` theorem boundary.
-/

/-- Kernel-checked verification certificate for `ZerocopySlice.EtherType_is_valid_bit_pattern`. -/
theorem verified : VerificationResult Obligations.ZerocopySlice.EtherType_is_valid_bit_pattern.claim :=
  Proof.ZerocopySlice.EtherType_is_valid_bit_pattern.verified

end Verify.ZerocopySlice.EtherType_is_valid_bit_pattern
