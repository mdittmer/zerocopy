-- Copyright 2026 The Fuchsia Authors
--
-- Licensed under a BSD-style license <LICENSE-BSD>, Apache License, Version 2.0
-- <LICENSE-APACHE or https://www.apache.org/licenses/LICENSE-2.0>, or the MIT
-- license <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your option.
-- This file may not be copied, modified, or distributed except according to
-- those terms.

prelude
import Init.Core
import Trusted
import Anneal
import Mir.ZerocopySlice.PacketHeader_total_packet_len
import Specs.ZerocopySlice.PacketHeader_total_packet_len
import Obligations.ZerocopySlice.PacketHeader_total_packet_len

namespace Proof.ZerocopySlice.PacketHeader_total_packet_len

open Trusted.Rustc.V2026_06_01.MirSemantics
open Trusted.VerificationResult
open Anneal.Rustc.V2026_06_01.Lower
open Anneal.Tactics
open Specs.ZerocopySlice.PacketHeader_total_packet_len

set_option maxHeartbeats 2000000 in
theorem verified :
    StatefulVerificationResult Obligations.ZerocopySlice.PacketHeader_total_packet_len.statefulClaim :=
  verifyStatefulByWPSymbolic
    Obligations.ZerocopySlice.PacketHeader_total_packet_len.statefulClaim
    32
    (fun ports : Value × Value =>
      packetValWithPayload100 ports.fst ports.snd :: .nil)
    (fun _ : Value × Value => emptyMemory)
    (fun _ : Value × Value => True)
    (fun _ _ ⟨ports, h_args, h_mem⟩ => ⟨ports, h_args, h_mem, True.intro⟩)
    (fun ports _ => ⟨ports, rfl, rfl, rfl⟩)

end Proof.ZerocopySlice.PacketHeader_total_packet_len
