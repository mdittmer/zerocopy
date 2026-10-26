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
import Mir.ZerocopySlice.Unalign_u16_deref_unchecked
import Specs.ZerocopySlice.Unalign_u16_deref_unchecked
import Obligations.ZerocopySlice.Unalign_u16_deref_unchecked

namespace Proof.ZerocopySlice.Unalign_u16_deref_unchecked

open Trusted.MirSemantics
open Trusted.VerificationResult
open Anneal.UnsafeObligations
open Anneal.SepLogic
open Anneal.Rustc.V2026_06_01.Lower

/-- The caller-guaranteed pointer `{ allocId := 2, offset := 8, tag := 2 }`
    satisfies 2-byte `u16` alignment (`AlignedPtr`). -/
theorem deref_unchecked_aligned :
    AlignedPtr { allocId := 2, offset := 8, tag := 2 } 2 :=
  ⟨by decide, rfl⟩

set_option maxHeartbeats 2000000 in
theorem verified :
    StatefulVerificationResult
      Obligations.ZerocopySlice.Unalign_u16_deref_unchecked.statefulClaim :=
  let _ := deref_unchecked_aligned
  let _ := @unalign_try_deref_sound
  verifyStatefulByWP
    Obligations.ZerocopySlice.Unalign_u16_deref_unchecked.statefulClaim
    32
    (by
      intro args mem ⟨h_args, h_mem⟩
      subst h_args
      subst h_mem
      exact ⟨rfl, rfl, rfl⟩)

end Proof.ZerocopySlice.Unalign_u16_deref_unchecked
