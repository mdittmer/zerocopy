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
import Obligations.ZerocopySlice.Unalign_u16_into_inner

namespace Specs.ZerocopySlice.Unalign_u16_into_inner

open Trusted.MirSemantics
open Trusted.VerificationResult

instance : ItemSpec "zerocopy_slice::Unalign::<u16>::into_inner" where
  pre args :=
    args =
      List.cons
        (.aggregate 0 (List.cons (.scalar { bits := 0x1234, sizeBytes := 2 }) List.nil))
        List.nil
  post _ ret :=
    ret = .scalar { bits := 0x1234, sizeBytes := 2 }

end Specs.ZerocopySlice.Unalign_u16_into_inner
