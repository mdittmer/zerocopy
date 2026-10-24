prelude
import Init.Core
import Trusted
import Mir.ZerocopySlice.PacketHeader_set_ports
import Obligations.ZerocopySlice.PacketHeader_set_ports
import Specs.ZerocopySlice.PacketHeader_set_ports
import Proof.ZerocopySlice.PacketHeader_set_ports

namespace Verify.ZerocopySlice.PacketHeader_set_ports

open Trusted.VerificationResult

/-!
Disposable verification assembly root for `ZerocopySlice.PacketHeader_set_ports`.

This module is generated on every `cargo anneal` run and assembles the
active `ItemSpec` and proof witness at the audited
`VerificationResult` theorem boundary.
-/

/-- Kernel-checked stateful verification certificate for `ZerocopySlice.PacketHeader_set_ports`. -/
theorem verified : StatefulVerificationResult Obligations.ZerocopySlice.PacketHeader_set_ports.statefulClaim :=
  Proof.ZerocopySlice.PacketHeader_set_ports.verified

end Verify.ZerocopySlice.PacketHeader_set_ports
