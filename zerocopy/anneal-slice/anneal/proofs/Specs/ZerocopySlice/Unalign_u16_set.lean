prelude
import Init.Core
import Trusted
import Obligations.ZerocopySlice.Unalign_u16_set

namespace Specs.ZerocopySlice.Unalign_u16_set

open Trusted.MirSemantics
open Trusted.VerificationResult

instance : StatefulItemSpec "zerocopy_slice::Unalign::<u16>::set" where
  pre args mem :=
    And
      (args =
        List.cons
          (.ptr { allocId := 1, offsetBytes := 1 } 8)
          (List.cons (.scalar { bits := 0x5678, sizeBytes := 2 }) List.nil))
      (mem =
        { emptyMemory with
          borrows :=
            { tag := 1
              targetLocal := 2
              targetProjection := .nil
              perm := .active } :: .nil })
  post _ _ ret mem' :=
    And
      (ret = .scalar { bits := 0x5678, sizeBytes := 2 })
      (lookupBorrow mem'.borrows 1 =
        .some
          { tag := 1
            targetLocal := 2
            targetProjection := .nil
            perm := .active })

end Specs.ZerocopySlice.Unalign_u16_set
