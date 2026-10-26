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
import Obligations.ZerocopySlice.Unalign_u16_new

namespace Specs.ZerocopySlice.Unalign_u16_new

open Trusted.MirAst
open Trusted.MirSemantics
open Trusted.VerificationResult

instance : ItemSpec "zerocopy_slice::Unalign::<u16>::new" where
  pre args :=
    Exists (fun val : ScalarInt =>
      And (val.sizeBytes = 2)
        (args = List.cons (.scalar val) List.nil))
  post args ret :=
    Exists (fun val : ScalarInt =>
      And (val.sizeBytes = 2)
        (And (args = List.cons (.scalar val) List.nil)
          (ret = .aggregate 0 (List.cons (.scalar val) List.nil))))

end Specs.ZerocopySlice.Unalign_u16_new
