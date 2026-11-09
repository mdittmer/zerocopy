prelude
import Init.Core
import Trusted
import Mir.ZerocopySlice.PacketHeader_total_packet_len
import Obligations.ZerocopySlice.PacketHeader_total_packet_len
import Specs.ZerocopySlice.PacketHeader_total_packet_len
import Proof.ZerocopySlice.PacketHeader_total_packet_len

namespace Verify.ZerocopySlice.PacketHeader_total_packet_len

open Trusted.VerificationResult

/-!
Disposable verification assembly root for `ZerocopySlice.PacketHeader_total_packet_len`.

This module is generated on every `cargo anneal` run and assembles the
active `ItemSpec` and proof witness at the audited
`VerificationResult` theorem boundary.
-/

/-- Kernel-checked stateful verification certificate for `ZerocopySlice.PacketHeader_total_packet_len`. -/
theorem verified : StatefulVerificationResult Obligations.ZerocopySlice.PacketHeader_total_packet_len.statefulClaim :=
  Proof.ZerocopySlice.PacketHeader_total_packet_len.verified

end Verify.ZerocopySlice.PacketHeader_total_packet_len
