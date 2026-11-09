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
import Mir.ZerocopySlice.Unalign_u8_deref
import Specs.ZerocopySlice.Unalign_u8_deref
import Obligations.ZerocopySlice.Unalign_u8_deref

namespace Proof.ZerocopySlice.Unalign_u8_deref

open Trusted.VerificationResult
open Anneal.Rustc.V2026_06_01.Lower

set_option maxHeartbeats 2000000 in
theorem verified : VerificationResult Obligations.ZerocopySlice.Unalign_u8_deref.claim :=
  verifyByWP Obligations.ZerocopySlice.Unalign_u8_deref.claim 32 (by
    intro args h_pre
    subst h_pre
    rfl)

end Proof.ZerocopySlice.Unalign_u8_deref
