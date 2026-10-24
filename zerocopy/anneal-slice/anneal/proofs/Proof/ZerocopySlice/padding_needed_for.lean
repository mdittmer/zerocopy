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
import Mir.ZerocopySlice.padding_needed_for
import Specs.ZerocopySlice.padding_needed_for
import Obligations.ZerocopySlice.padding_needed_for

namespace Proof.ZerocopySlice.padding_needed_for

open Trusted.MirSemantics
open Trusted.VerificationResult
open Anneal.Rustc.V2026_06_01.Lower

set_option maxHeartbeats 2000000 in
theorem verified :
    StatefulVerificationResult
      Obligations.ZerocopySlice.padding_needed_for.statefulClaim :=
  verifyStatefulByWP
    Obligations.ZerocopySlice.padding_needed_for.statefulClaim
    32
    (by
      intro args mem ⟨h_args, h_mem⟩
      subst h_args
      subst h_mem
      rfl)

end Proof.ZerocopySlice.padding_needed_for
