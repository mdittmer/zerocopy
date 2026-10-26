prelude
import Init.Core
import Trusted

namespace Mir.ZerocopySlice.__anneal_unsafe_impl_Unaligned_for_BigEndianU16

open Trusted.Rustc.V2026_06_01.Layout
open Trusted.Rustc.V2026_06_01.MirAst
open Trusted.Rustc.V2026_06_01.MirSemantics
open Trusted.VerificationResult

/-! Captured `Runtime(PostCleanup)` MIR and compiler layout table for `zerocopy_slice::__anneal_unsafe_impl_Unaligned_for_BigEndianU16`. -/

-- TypeLayout[0]: size=1, align=1
def layouts : LayoutTable :=
  { pointerBits := 64, isLittleEndian := true, entries := (List.cons { size := { bytes := 1 }, align := { bytes := 1 }, backendRepr := (BackendRepr.scalar (ScalarLayout.initialized (PrimitiveKind.int 8 false) { start := 0, end_ := 1 })), fields := FieldsShape.primitive, variants := (VariantsShape.single 0), isUninhabited := false } List.nil) }

-- bb0: _0 = const true; return
def body : CapturedMir :=
  {
    defPath := "zerocopy_slice::__anneal_unsafe_impl_Unaligned_for_BigEndianU16",
    phase := MirPhaseTag.runtimeOptimized,
    role := ContractRole.subject,
    argCount := 0,
    localDecls := (List.cons { mutability := Mutability.mut, ty := { kind := RigidTy.bool, layoutIdx := 0 }, debugName := Option.none } List.nil),
    basicBlocks := (List.cons { statements := (List.cons { kind := (StatementKind.assign { local_ := 0, projection := List.nil } (Rvalue.use (Operand.constant { ty := { kind := RigidTy.bool, layoutIdx := 0 }, val := (ConstValue.scalar (ConstScalar.int { bits := 1, sizeBytes := 1 })) }))), span := "src/lib.rs:204:28: 204:32" } List.nil), terminator := { kind := TerminatorKind.return_, span := "src/lib.rs:204:34: 204:34" }, isCleanup := false } List.nil),
    fnAbi := { args := List.nil, ret := { layout := { size := { bytes := 1 }, align := { bytes := 1 }, backendRepr := (BackendRepr.scalar (ScalarLayout.initialized (PrimitiveKind.int 8 false) { start := 0, end_ := 1 })), fields := FieldsShape.primitive, variants := (VariantsShape.single 0), isUninhabited := false }, mode := AbiPassMode.direct }, conv := CallConv.rust, canUnwind := true },
    layouts := layouts,
    allocations := List.nil
  }

def defaultClaim : Claim :=
  { body := body, pre := fun _ => True, post := fun _ _ => True }

end Mir.ZerocopySlice.__anneal_unsafe_impl_Unaligned_for_BigEndianU16
