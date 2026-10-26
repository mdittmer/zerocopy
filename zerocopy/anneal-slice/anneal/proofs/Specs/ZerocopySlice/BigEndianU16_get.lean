prelude
import Init.Core
import Trusted
import Obligations.ZerocopySlice.BigEndianU16_get

namespace Specs.ZerocopySlice.BigEndianU16_get

open Trusted.MirSemantics
open Trusted.VerificationResult

instance : ItemSpec "zerocopy_slice::BigEndianU16::get" where
  pre args :=
    args =
      List.cons
        (.aggregate 0
          (List.cons
            (.aggregate 0
              (List.cons (.scalar { bits := 0x12, sizeBytes := 1 })
                (List.cons (.scalar { bits := 0x34, sizeBytes := 1 }) List.nil)))
            List.nil))
        List.nil
  post _ ret :=
    ret = .scalar { bits := 0x1234, sizeBytes := 2 }

end Specs.ZerocopySlice.BigEndianU16_get
