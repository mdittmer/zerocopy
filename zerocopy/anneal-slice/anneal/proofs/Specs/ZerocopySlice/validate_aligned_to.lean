prelude
import Init.Core
import Trusted
import Obligations.ZerocopySlice.validate_aligned_to

namespace Specs.ZerocopySlice.validate_aligned_to

open Trusted.MirSemantics
open Trusted.VerificationResult

instance : ItemSpec "zerocopy_slice::validate_aligned_to" where
  pre args :=
    args =
      List.cons (.scalar { bits := 24, sizeBytes := 8 })
        (List.cons (.scalar { bits := 8, sizeBytes := 8 }) List.nil)
  post _ ret :=
    ret = .scalar { bits := 1, sizeBytes := 1 }

end Specs.ZerocopySlice.validate_aligned_to
