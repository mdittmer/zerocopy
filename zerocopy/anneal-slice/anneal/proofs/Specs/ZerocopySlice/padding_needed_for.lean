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
import Obligations.ZerocopySlice.padding_needed_for

namespace Specs.ZerocopySlice.padding_needed_for

open Trusted.MirSemantics
open Trusted.VerificationResult

/-- Assumed external contract for `core::num::<impl usize>::wrapping_sub` on
    64-bit `usize` inputs. -/
def wrappingSubContract : CalleeContractEntry where
  defPath := "core::num::<impl usize>::wrapping_sub"
  pre := fun args =>
    match args with
    | .scalar a :: .scalar b :: .nil =>
      Bool.and (Nat.beq a.sizeBytes 8) (Nat.beq b.sizeBytes 8)
    | _ => false
  ret := fun args =>
    match args with
    | .scalar a :: .scalar b :: .nil =>
      .scalar {
        bits :=
          match Nat.ble b.bits a.bits with
          | true => Nat.sub a.bits b.bits
          | false =>
            Nat.sub (Nat.add 18446744073709551616 a.bits) b.bits
        sizeBytes := 8
      }
    | _ => .unit

instance : StatefulItemSpec "zerocopy_slice::padding_needed_for" where
  pre args mem :=
    And
      (args =
        List.cons (.scalar { bits := 300, sizeBytes := 8 })
          (List.cons (.scalar { bits := 32, sizeBytes := 8 }) List.nil))
      (mem = { emptyMemory with calleeContracts := wrappingSubContract :: .nil })
  post _ _ ret _ :=
    ret = .scalar { bits := 20, sizeBytes := 8 }

end Specs.ZerocopySlice.padding_needed_for
