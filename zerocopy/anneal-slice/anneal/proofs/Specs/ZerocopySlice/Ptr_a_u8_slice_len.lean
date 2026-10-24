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
import Obligations.ZerocopySlice.Ptr_a_u8_slice_len

namespace Specs.ZerocopySlice.Ptr_a_u8_slice_len

open Trusted.Rustc.V2026_06_01.MirSemantics
open Trusted.VerificationResult

def sliceLenContract : CalleeContractEntry where
  defPath := "core::ptr::const_ptr::<impl *const [T]>::len"
  pre := fun args =>
    match args with
    | .aggregate 0 (.ptr _ _ :: .scalar _ :: .nil) :: .nil => true
    | _ => false
  ret := fun args =>
    match args with
    | .aggregate 0 (.ptr _ _ :: .scalar metaSc :: .nil) :: .nil =>
      .scalar { bits := metaSc.bits, sizeBytes := 8 }
    | _ => .unit

def slicePtrArg (len : Nat) : Value :=
  .aggregate 0
    ( .aggregate 0
        ( .ptr { allocId := 0, offsetBytes := 0 } 8 ::
          .scalar { bits := len, sizeBytes := 8 } ::
          .nil ) ::
      .aggregate 0 .nil ::
      .nil )

instance : StatefulItemSpec "zerocopy_slice::Ptr::<'a, [u8]>::slice_len" where
  pre args mem :=
    Exists (fun len : Nat =>
      And
        (args = slicePtrArg len :: .nil)
        (mem = { emptyMemory with calleeContracts := sliceLenContract :: .nil }))
  post args _ ret _ :=
    Exists (fun len : Nat =>
      And
        (args = slicePtrArg len :: .nil)
        (ret = .scalar { bits := len, sizeBytes := 8 }))

end Specs.ZerocopySlice.Ptr_a_u8_slice_len
