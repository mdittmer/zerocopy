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
import Obligations.ZerocopySlice.round_down_to_next_multiple_of_alignment

namespace Specs.ZerocopySlice.round_down_to_next_multiple_of_alignment

open Trusted.MirSemantics
open Trusted.VerificationResult

instance : ItemSpec "zerocopy_slice::round_down_to_next_multiple_of_alignment" where
  pre args :=
    args =
      List.cons (.scalar { bits := 23, sizeBytes := 8 })
        (List.cons (.scalar { bits := 8, sizeBytes := 8 }) List.nil)
  post _ ret :=
    ret = .scalar { bits := 16, sizeBytes := 8 }

end Specs.ZerocopySlice.round_down_to_next_multiple_of_alignment
