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
import Mir.ZerocopySlice.__anneal_unsafe_impl_FromBytes_for_PacketHeader
import Mir.ZerocopySlice.EtherType___anneal_type_invariant_ether_type
import Mir.ZerocopySlice.PacketHeader___anneal_type_invariant_packet_header
import Obligations.ZerocopySlice.__anneal_unsafe_impl_FromBytes_for_PacketHeader

namespace Proof.ZerocopySlice.__anneal_unsafe_impl_FromBytes_for_PacketHeader

open Trusted.VerificationResult
open Anneal.BitValidity
open Anneal.UnsafeObligations
open Anneal.SepLogic
open Anneal.Rustc.V2026_06_01.Lower

theorem verified :
    VerificationResult Obligations.ZerocopySlice.__anneal_unsafe_impl_FromBytes_for_PacketHeader.claim :=
  let _ := @allBytesInitialized_nil
  let _ := @AlignedPtr
  let _ := @sepConj_comm
  verifyByWP Obligations.ZerocopySlice.__anneal_unsafe_impl_FromBytes_for_PacketHeader.claim 16 (by
    intro _ _
    exact True.intro)

end Proof.ZerocopySlice.__anneal_unsafe_impl_FromBytes_for_PacketHeader
