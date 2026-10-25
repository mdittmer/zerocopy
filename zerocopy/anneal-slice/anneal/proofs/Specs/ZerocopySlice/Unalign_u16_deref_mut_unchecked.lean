prelude
import Init.Core
import Trusted
import Obligations.ZerocopySlice.Unalign_u16_deref_mut_unchecked

namespace Specs.ZerocopySlice.Unalign_u16_deref_mut_unchecked

open Trusted.MirSemantics
open Trusted.VerificationResult

instance : StatefulItemSpec "zerocopy_slice::Unalign::<u16>::deref_mut_unchecked" where
  pre args mem :=
    And
      (args =
        List.cons
          (.ptr { allocId := 2, offsetBytes := 8 } 8)
          List.nil)
      (mem =
        { emptyMemory with
          borrows :=
            { tag := 2
              targetLocal := 2
              targetProjection := .nil
              perm := .active
              rangeOffset := 8
              rangeLen := 2
              protector := .some 1 } :: .nil
          activeProtectors := 1 :: .nil })
  post _ _ ret mem' :=
    And
      (ret = .ptr { allocId := 2, offsetBytes := 8 } 8)
      (And
        (lookupBorrow mem'.borrows 2 =
          .some
            { tag := 2
              targetLocal := 2
              targetProjection := .nil
              perm := .active
              rangeOffset := 8
              rangeLen := 2
              protector := .some 1 })
        (mem'.activeProtectors = 1 :: .nil))

end Specs.ZerocopySlice.Unalign_u16_deref_mut_unchecked
