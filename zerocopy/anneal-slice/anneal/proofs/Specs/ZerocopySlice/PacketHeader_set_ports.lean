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
import Obligations.ZerocopySlice.PacketHeader_set_ports

namespace Specs.ZerocopySlice.PacketHeader_set_ports

open Trusted.MirSemantics
open Trusted.VerificationResult

def initialHeader : Value :=
  .aggregate 0
    (List.cons (.aggregate 0 (List.cons (.scalar { bits := 80, sizeBytes := 2 }) List.nil))
      (List.cons (.aggregate 0 (List.cons (.scalar { bits := 8080, sizeBytes := 2 }) List.nil))
        (List.cons (.aggregate 0 (List.cons (.scalar { bits := 512, sizeBytes := 2 }) List.nil))
          List.nil)))

def updatedHeader : Value :=
  .aggregate 0
    (List.cons (.aggregate 0 (List.cons (.scalar { bits := 443, sizeBytes := 2 }) List.nil))
      (List.cons (.aggregate 0 (List.cons (.scalar { bits := 8443, sizeBytes := 2 }) List.nil))
        (List.cons (.aggregate 0 (List.cons (.scalar { bits := 512, sizeBytes := 2 }) List.nil))
          List.nil)))

instance : StatefulItemSpec "zerocopy_slice::PacketHeader::set_ports" where
  pre args mem :=
    And
      (args =
        List.cons initialHeader
          (List.cons (.scalar { bits := 443, sizeBytes := 2 })
            (List.cons (.scalar { bits := 8443, sizeBytes := 2 }) List.nil)))
      (mem = emptyMemory)
  post _ _ ret _ :=
    ret = updatedHeader

end Specs.ZerocopySlice.PacketHeader_set_ports
