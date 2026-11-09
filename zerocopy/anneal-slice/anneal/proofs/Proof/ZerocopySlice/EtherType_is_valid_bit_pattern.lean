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
import Mir.ZerocopySlice.EtherType_is_valid_bit_pattern
import Specs.ZerocopySlice.EtherType_is_valid_bit_pattern
import Obligations.ZerocopySlice.EtherType_is_valid_bit_pattern

namespace Proof.ZerocopySlice.EtherType_is_valid_bit_pattern

open Trusted.Rustc.V2026_06_01.MirSemantics
open Trusted.VerificationResult
open Anneal.Rustc.V2026_06_01.Lower
open Anneal.Tactics

def ipv6ValidClaim : Claim where
  body := Mir.ZerocopySlice.EtherType_is_valid_bit_pattern.body
  pre := fun args => args = .scalar { bits := 0x86DD, sizeBytes := 2 } :: .nil
  post := fun _ ret => ret = .scalar { bits := 1, sizeBytes := 1 }

def invalidDiscriminantRejectedClaim : Claim where
  body := Mir.ZerocopySlice.EtherType_is_valid_bit_pattern.body
  pre := fun args => args = .scalar { bits := 0x0000, sizeBytes := 2 } :: .nil
  post := fun _ ret => ret = .scalar { bits := 0, sizeBytes := 1 }

set_option maxHeartbeats 2000000 in
theorem ipv6_valid : VerificationResult ipv6ValidClaim :=
  dischargeStatelessEq
    ipv6ValidClaim
    32
    (.scalar { bits := 0x86DD, sizeBytes := 2 } :: .nil)
    (fun _ h => h)
    rfl

set_option maxHeartbeats 2000000 in
theorem invalid_discriminant_rejected :
    VerificationResult invalidDiscriminantRejectedClaim :=
  dischargeStatelessEq
    invalidDiscriminantRejectedClaim
    32
    (.scalar { bits := 0x0000, sizeBytes := 2 } :: .nil)
    (fun _ h => h)
    rfl

set_option maxHeartbeats 2000000 in
theorem verified :
    VerificationResult Obligations.ZerocopySlice.EtherType_is_valid_bit_pattern.claim :=
  dischargeStatelessEq
    Obligations.ZerocopySlice.EtherType_is_valid_bit_pattern.claim
    32
    (.scalar { bits := 0x0800, sizeBytes := 2 } :: .nil)
    (fun _ h => h)
    rfl

end Proof.ZerocopySlice.EtherType_is_valid_bit_pattern
