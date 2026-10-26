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
import Mir.ZerocopySlice.Unalign_u16_new
import Specs.ZerocopySlice.Unalign_u16_new
import Obligations.ZerocopySlice.Unalign_u16_new

namespace Proof.ZerocopySlice.Unalign_u16_new

open Trusted.VerificationResult
open Anneal.Rustc.V2026_06_01.Lower
open Anneal.Tactics
open Anneal.Polymorphism

theorem parametric_inst_witness :
    VerificationResult (parametricUnalignNewClaim { size := 2, align := 2 }) :=
  unalign_new_u16_inst

theorem verified : VerificationResult Obligations.ZerocopySlice.Unalign_u16_new.claim :=
  dischargeSymbolicScalarArg
    Obligations.ZerocopySlice.Unalign_u16_new.claim
    16
    (fun val => val.sizeBytes = 2)
    (fun _ ⟨val, h_sz, h_args⟩ => ⟨val, h_args, h_sz⟩)
    (fun val h_sz => ⟨val, h_sz, rfl, rfl⟩)

end Proof.ZerocopySlice.Unalign_u16_new
