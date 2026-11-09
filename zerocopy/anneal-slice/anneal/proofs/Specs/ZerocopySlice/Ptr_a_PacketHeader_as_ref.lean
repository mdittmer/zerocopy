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
import Obligations.ZerocopySlice.Ptr_a_PacketHeader_as_ref

namespace Specs.ZerocopySlice.Ptr_a_PacketHeader_as_ref

open Trusted.MirSemantics
open Trusted.VerificationResult

instance : StatefulItemSpec "zerocopy_slice::Ptr::<'a, PacketHeader>::as_ref" where
  pre args mem :=
    And
      (args =
        List.cons
          (.aggregate 0
            (List.cons
              (.ptr { allocId := 3, offsetBytes := 0 } 8)
              (List.cons .unit List.nil)))
          List.nil)
      (mem =
        { emptyMemory with
          borrows :=
            { tag := 3
              targetLocal := 3
              targetProjection := .nil
              perm := .frozen
              rangeOffset := 0
              rangeLen := 6
              protector := .some 1 } :: .nil
          activeProtectors := 1 :: .nil })
  post _ _ ret mem' :=
    And
      (ret = .ptr { allocId := 3, offsetBytes := 0 } 8)
      (And
        (lookupBorrow mem'.borrows 3 =
          .some
            { tag := 3
              targetLocal := 3
              targetProjection := .nil
              perm := .frozen
              rangeOffset := 0
              rangeLen := 6
              protector := .some 1 })
        (mem'.activeProtectors = 1 :: .nil))

end Specs.ZerocopySlice.Ptr_a_PacketHeader_as_ref
