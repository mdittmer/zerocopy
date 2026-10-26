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
import Obligations.ZerocopySlice.PacketHeader_total_packet_len

namespace Specs.ZerocopySlice.PacketHeader_total_packet_len

open Trusted.Rustc.V2026_06_01.MirAst
open Trusted.Rustc.V2026_06_01.MirSemantics
open Trusted.VerificationResult

def packetValWithPayload100 (src dst : Value) : Value :=
  .aggregate 0
    ( .aggregate 0 (src :: .nil) ::
      .aggregate 0 (dst :: .nil) ::
      .aggregate 0 (.scalar { bits := 100, sizeBytes := 2 } :: .nil) ::
      .nil )

def expectedPayloadBorrow : BorrowEntry :=
  { tag := 1
    targetLocal := 1
    targetProjection :=
      .field 2
        { kind :=
            .adt "zerocopy_slice::Unalign" (.uint .u16 :: .nil)
          layoutIdx := 4 } ::
        .nil
    perm := .frozen }

instance : StatefulItemSpec "zerocopy_slice::PacketHeader::total_packet_len" where
  pre args mem :=
    Exists (fun ports : Value × Value =>
      And
        (args = packetValWithPayload100 ports.fst ports.snd :: .nil)
        (mem = emptyMemory))
  post args _ ret mem' :=
    Exists (fun ports : Value × Value =>
      And
        (args = packetValWithPayload100 ports.fst ports.snd :: .nil)
        (And
          (ret = .scalar { bits := 106, sizeBytes := 2 })
          (lookupBorrow mem'.borrows 1 = .some expectedPayloadBorrow)))

end Specs.ZerocopySlice.PacketHeader_total_packet_len
