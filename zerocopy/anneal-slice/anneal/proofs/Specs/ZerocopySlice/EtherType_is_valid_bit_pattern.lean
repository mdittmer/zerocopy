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
import Obligations.ZerocopySlice.EtherType_is_valid_bit_pattern

namespace Specs.ZerocopySlice.EtherType_is_valid_bit_pattern

open Trusted.Rustc.V2026_06_01.MirSemantics
open Trusted.VerificationResult

instance : ItemSpec "zerocopy_slice::EtherType::is_valid_bit_pattern" where
  pre args :=
    args = .scalar { bits := 0x0800, sizeBytes := 2 } :: .nil
  post _ ret :=
    ret = .scalar { bits := 1, sizeBytes := 1 }

end Specs.ZerocopySlice.EtherType_is_valid_bit_pattern
