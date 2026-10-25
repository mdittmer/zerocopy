prelude
import Init.Core
import Trusted

namespace Mir.ZerocopySlice.validate_aligned_to

open Trusted.Rustc.V2026_06_01.Layout
open Trusted.Rustc.V2026_06_01.MirAst
open Trusted.Rustc.V2026_06_01.MirSemantics
open Trusted.VerificationResult

/-! Captured `Runtime(PostCleanup)` MIR and compiler layout table for `zerocopy_slice::validate_aligned_to`. -/

-- TypeLayout[0]: size=1, align=1
-- TypeLayout[1]: size=8, align=8
-- TypeLayout[2]: size=16, align=8
def layouts : LayoutTable :=
  { pointerBits := 64, isLittleEndian := true, entries := (List.cons { size := { bytes := 1 }, align := { bytes := 1 }, backendRepr := (BackendRepr.scalar (ScalarLayout.initialized (PrimitiveKind.int 8 false) { start := 0, end_ := 1 })), fields := FieldsShape.primitive, variants := (VariantsShape.single 0), isUninhabited := false } (List.cons { size := { bytes := 8 }, align := { bytes := 8 }, backendRepr := (BackendRepr.scalar (ScalarLayout.initialized (PrimitiveKind.int 64 false) { start := 0, end_ := 18446744073709551615 })), fields := FieldsShape.primitive, variants := (VariantsShape.single 0), isUninhabited := false } (List.cons { size := { bytes := 16 }, align := { bytes := 8 }, backendRepr := (BackendRepr.scalarPair (ScalarLayout.initialized (PrimitiveKind.int 64 false) { start := 0, end_ := 18446744073709551615 }) (ScalarLayout.initialized (PrimitiveKind.int 8 false) { start := 0, end_ := 1 })), fields := (FieldsShape.arbitrary (List.cons { bytes := 0 } (List.cons { bytes := 8 } List.nil)) (List.cons 0 (List.cons 1 List.nil))), variants := (VariantsShape.single 0), isUninhabited := false } List.nil))) }

-- bb0: _4 = SubWithOverflow(copy _2, const 1_usize); assert(!move (_4.1: bool), "attempt to compute `{} - {}`, which would overflow", copy _2, const 1_usize) -> [success: bb1, unwind continue]
-- bb1: _3 = move (_4.0: usize); _5 = BitAnd(copy _1, copy _3); _0 = Eq(move _5, const 0_usize); return
def body : CapturedMir :=
  {
    defPath := "zerocopy_slice::validate_aligned_to",
    phase := MirPhaseTag.runtimeOptimized,
    role := ContractRole.subject,
    argCount := 2,
    localDecls := (List.cons { mutability := Mutability.mut, ty := { kind := RigidTy.bool, layoutIdx := 0 }, debugName := Option.none } (List.cons { mutability := Mutability.not, ty := { kind := (RigidTy.uint UintTy.usize), layoutIdx := 1 }, debugName := (Option.some "addr") } (List.cons { mutability := Mutability.not, ty := { kind := (RigidTy.uint UintTy.usize), layoutIdx := 1 }, debugName := (Option.some "align") } (List.cons { mutability := Mutability.not, ty := { kind := (RigidTy.uint UintTy.usize), layoutIdx := 1 }, debugName := (Option.some "mask") } (List.cons { mutability := Mutability.mut, ty := { kind := (RigidTy.tuple (List.cons (RigidTy.uint UintTy.usize) (List.cons RigidTy.bool List.nil))), layoutIdx := 2 }, debugName := Option.none } (List.cons { mutability := Mutability.mut, ty := { kind := (RigidTy.uint UintTy.usize), layoutIdx := 1 }, debugName := Option.none } List.nil)))))),
    basicBlocks := (List.cons { statements := (List.cons { kind := (StatementKind.storageLive 3), span := "<dummy>" } (List.cons { kind := (StatementKind.storageLive 4), span := "<dummy>" } (List.cons { kind := (StatementKind.storageLive 5), span := "<dummy>" } (List.cons { kind := (StatementKind.assign { local_ := 4, projection := List.nil } (Rvalue.binaryOp BinOp.subWithOverflow (Operand.copy { local_ := 2, projection := List.nil }) (Operand.constant { ty := { kind := (RigidTy.uint UintTy.usize), layoutIdx := 1 }, val := (ConstValue.scalar (ConstScalar.int { bits := 1, sizeBytes := 8 })) }))), span := "src/lib.rs:161:16: 161:25" } List.nil)))), terminator := { kind := (TerminatorKind.assert (Operand.move { local_ := 4, projection := (List.cons (ProjectionElem.field 1 { kind := RigidTy.bool, layoutIdx := 0 }) List.nil) }) false 1 UnwindAction.continue_), span := "src/lib.rs:161:16: 161:25" }, isCleanup := false } (List.cons { statements := (List.cons { kind := (StatementKind.assign { local_ := 3, projection := List.nil } (Rvalue.use (Operand.move { local_ := 4, projection := (List.cons (ProjectionElem.field 0 { kind := (RigidTy.uint UintTy.usize), layoutIdx := 1 }) List.nil) }))), span := "src/lib.rs:161:16: 161:25" } (List.cons { kind := (StatementKind.assign { local_ := 5, projection := List.nil } (Rvalue.binaryOp BinOp.bitAnd (Operand.copy { local_ := 1, projection := List.nil }) (Operand.copy { local_ := 3, projection := List.nil }))), span := "src/lib.rs:162:5: 162:18" } (List.cons { kind := (StatementKind.assign { local_ := 0, projection := List.nil } (Rvalue.binaryOp BinOp.eq (Operand.move { local_ := 5, projection := List.nil }) (Operand.constant { ty := { kind := (RigidTy.uint UintTy.usize), layoutIdx := 1 }, val := (ConstValue.scalar (ConstScalar.int { bits := 0, sizeBytes := 8 })) }))), span := "src/lib.rs:162:5: 162:23" } List.nil))), terminator := { kind := TerminatorKind.return_, span := "src/lib.rs:163:2: 163:2" }, isCleanup := false } List.nil)),
    fnAbi := { args := (List.cons { layout := { size := { bytes := 8 }, align := { bytes := 8 }, backendRepr := (BackendRepr.scalar (ScalarLayout.initialized (PrimitiveKind.int 64 false) { start := 0, end_ := 18446744073709551615 })), fields := FieldsShape.primitive, variants := (VariantsShape.single 0), isUninhabited := false }, mode := AbiPassMode.direct } (List.cons { layout := { size := { bytes := 8 }, align := { bytes := 8 }, backendRepr := (BackendRepr.scalar (ScalarLayout.initialized (PrimitiveKind.int 64 false) { start := 0, end_ := 18446744073709551615 })), fields := FieldsShape.primitive, variants := (VariantsShape.single 0), isUninhabited := false }, mode := AbiPassMode.direct } List.nil)), ret := { layout := { size := { bytes := 1 }, align := { bytes := 1 }, backendRepr := (BackendRepr.scalar (ScalarLayout.initialized (PrimitiveKind.int 8 false) { start := 0, end_ := 1 })), fields := FieldsShape.primitive, variants := (VariantsShape.single 0), isUninhabited := false }, mode := AbiPassMode.direct }, conv := CallConv.rust, canUnwind := true },
    layouts := layouts,
    allocations := List.nil
  }

def defaultClaim : Claim :=
  { body := body, pre := fun _ => True, post := fun _ _ => True }

end Mir.ZerocopySlice.validate_aligned_to
