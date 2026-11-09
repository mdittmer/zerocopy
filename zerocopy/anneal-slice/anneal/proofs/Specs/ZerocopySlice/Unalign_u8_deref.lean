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
import Obligations.ZerocopySlice.Unalign_u8_deref

namespace Specs.ZerocopySlice.Unalign_u8_deref

open Trusted.MirSemantics
open Trusted.VerificationResult

instance : ItemSpec "zerocopy_slice::Unalign::<u8>::deref" where
  pre args :=
    args =
      List.cons
        (.ptr { allocId := 1, offsetBytes := 4 } 8)
        List.nil
  post _ ret :=
    ret = .ptr { allocId := 1, offsetBytes := 4 } 8

end Specs.ZerocopySlice.Unalign_u8_deref
