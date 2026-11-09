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
import Obligations.ZerocopySlice.transmute_unchecked

namespace Specs.ZerocopySlice.transmute_unchecked

open Trusted.MirSemantics
open Trusted.VerificationResult

instance : ItemSpec "zerocopy_slice::transmute_unchecked" where
  pre args :=
    args = List.cons (.scalar { bits := 0x0102, sizeBytes := 2 }) List.nil
  post _ ret :=
    ret =
      .aggregate 0
        (List.cons (.scalar { bits := 0x02, sizeBytes := 1 })
          (List.cons (.scalar { bits := 0x01, sizeBytes := 1 }) List.nil))

end Specs.ZerocopySlice.transmute_unchecked
