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
import Mir.ZerocopySlice.PacketHeader_set_ports
import Specs.ZerocopySlice.PacketHeader_set_ports
import Obligations.ZerocopySlice.PacketHeader_set_ports

namespace Proof.ZerocopySlice.PacketHeader_set_ports

open Trusted.MirSemantics
open Trusted.VerificationResult
open Anneal.Memory
open Anneal.SepLogic
open Anneal.Rustc.V2026_06_01.Lower

set_option maxHeartbeats 2000000 in
theorem verified :
    StatefulVerificationResult
      Obligations.ZerocopySlice.PacketHeader_set_ports.statefulClaim :=
  let _ := @disjoint_byte_range_operational_non_interference
  let _ := @pointsToSep_star_exclusive
  let _ := @wp_frame_disjoint
  verifyStatefulByWP
    Obligations.ZerocopySlice.PacketHeader_set_ports.statefulClaim
    64
    (by
      intro args mem ⟨h_args, h_mem⟩
      subst h_args
      subst h_mem
      rfl)

end Proof.ZerocopySlice.PacketHeader_set_ports
