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
import Mir.ZerocopySlice.Ptr_a_u8_slice_len
import Specs.ZerocopySlice.Ptr_a_u8_slice_len
import Obligations.ZerocopySlice.Ptr_a_u8_slice_len

namespace Proof.ZerocopySlice.Ptr_a_u8_slice_len

open Trusted.Rustc.V2026_06_01.MirSemantics
open Trusted.VerificationResult
open Anneal.Rustc.V2026_06_01.Lower
open Anneal.Tactics
open Specs.ZerocopySlice.Ptr_a_u8_slice_len

set_option maxHeartbeats 2000000 in
theorem verified :
    StatefulVerificationResult Obligations.ZerocopySlice.Ptr_a_u8_slice_len.statefulClaim :=
  verifyStatefulByWPSymbolic
    Obligations.ZerocopySlice.Ptr_a_u8_slice_len.statefulClaim
    16
    (fun len : Nat => slicePtrArg len :: .nil)
    (fun _ : Nat => { emptyMemory with calleeContracts := sliceLenContract :: .nil })
    (fun _ : Nat => True)
    (fun _ _ ⟨len, h_args, h_mem⟩ => ⟨len, h_args, h_mem, True.intro⟩)
    (fun len _ => ⟨len, rfl, rfl⟩)

end Proof.ZerocopySlice.Ptr_a_u8_slice_len
