prelude
import Init.Core
import Trusted
import Anneal
import Mir.ZerocopySlice.__anneal_unsafe_impl_Unaligned_for_BigEndianU16
import Obligations.ZerocopySlice.__anneal_unsafe_impl_Unaligned_for_BigEndianU16

namespace Proof.ZerocopySlice.__anneal_unsafe_impl_Unaligned_for_BigEndianU16

open Trusted.VerificationResult
open Anneal.BitValidity
open Anneal.Polymorphism
open Anneal.Rustc.V2026_06_01.Lower

theorem byteorder_u16_traits_witness :
    IsUnaligned byteOrderLayoutTable byteOrderU16Ty ∧
    IsFromBytes byteOrderLayoutTable byteOrderU16Ty ∧
    IsIntoBytes byteOrderLayoutTable byteOrderU16Ty ∧
    IsFromZeros byteOrderLayoutTable byteOrderU16Ty ∧
    IsUnalignedDesc byteOrderU16LayoutDesc :=
  ⟨byteOrderU16_isUnaligned,
   byteOrderU16_isFromBytes,
   byteOrderU16_isIntoBytes,
   byteOrderU16_isFromZeros,
   byteOrderU16LayoutDesc_unaligned⟩

theorem verified :
    VerificationResult
      Obligations.ZerocopySlice.__anneal_unsafe_impl_Unaligned_for_BigEndianU16.claim :=
  let _ := byteorder_u16_traits_witness
  let _ := @array_u8_2_isUnaligned
  verifyByWP
    Obligations.ZerocopySlice.__anneal_unsafe_impl_Unaligned_for_BigEndianU16.claim
    16
    (by
      intro _ _
      exact True.intro)

end Proof.ZerocopySlice.__anneal_unsafe_impl_Unaligned_for_BigEndianU16
