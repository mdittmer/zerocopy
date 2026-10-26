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
import Anneal
import Mir.ZerocopySlice.Ptr_a_PacketHeader_as_ref
import Specs.ZerocopySlice.Ptr_a_PacketHeader_as_ref
import Obligations.ZerocopySlice.Ptr_a_PacketHeader_as_ref

namespace Proof.ZerocopySlice.Ptr_a_PacketHeader_as_ref

open Trusted.MirSemantics
open Trusted.VerificationResult
open Anneal.Memory
open Anneal.UnsafeObligations
open Anneal.SepLogic
open Anneal.Rustc.V2026_06_01.Lower

/-- Initial borrow entry for `Ptr<'a, PacketHeader>::as_ref` carries a 6-byte
    `PacketHeader` byte range `[0, 6)` and active lifetime protector `1`. -/
def initialPtrBorrowEntry : BorrowEntry where
  tag := 3
  targetLocal := 3
  targetProjection := .nil
  perm := .frozen
  rangeOffset := 0
  rangeLen := 6
  protector := .some 1

theorem initialPtrBorrow_isProtected :
    (BorrowEntry.toByteRangePerm initialPtrBorrowEntry).isProtected
      (ProtectorSet.ofMemory
        { emptyMemory with
          borrows := initialPtrBorrowEntry :: .nil
          activeProtectors := 1 :: .nil }) = true :=
  rfl

theorem initialPtr_borrowTokenSep :
    BorrowTokenSep { allocId := 3, offset := 0, tag := 3 } .frozen
      { state :=
          { emptyMemory with
            borrows := initialPtrBorrowEntry :: .nil
            activeProtectors := 1 :: .nil }
        footprint := { addrs := .nil, tags := 3 :: .nil } } :=
  ⟨⟨initialPtrBorrowEntry, rfl, rfl, rfl⟩, rfl, rfl⟩

set_option maxHeartbeats 2000000 in
theorem verified :
    StatefulVerificationResult
      Obligations.ZerocopySlice.Ptr_a_PacketHeader_as_ref.statefulClaim :=
  verifyStatefulByWP
    Obligations.ZerocopySlice.Ptr_a_PacketHeader_as_ref.statefulClaim
    32
    (by
      intro args mem ⟨h_args, h_mem⟩
      subst h_args
      subst h_mem
      exact ⟨rfl, rfl, rfl⟩)

end Proof.ZerocopySlice.Ptr_a_PacketHeader_as_ref
