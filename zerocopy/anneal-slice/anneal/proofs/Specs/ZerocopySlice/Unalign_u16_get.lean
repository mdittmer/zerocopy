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
import Obligations.ZerocopySlice.Unalign_u16_get

namespace Specs.ZerocopySlice.Unalign_u16_get

open Trusted.MirSemantics
open Trusted.VerificationResult

instance : StatefulItemSpec "zerocopy_slice::Unalign::<u16>::get" where
  pre args mem :=
    And
      (args =
        List.cons
          (.ptr { allocId := 1, offsetBytes := 0 } 8)
          List.nil)
      (mem = emptyMemory)
  post _ _ ret mem' :=
    And
      (ret = .scalar { bits := 0x1234, sizeBytes := 2 })
      (lookupBorrow mem'.borrows 1 =
        .some
          { tag := 1
            targetLocal := 2
            targetProjection := .nil
            perm := .frozen })

end Specs.ZerocopySlice.Unalign_u16_get
