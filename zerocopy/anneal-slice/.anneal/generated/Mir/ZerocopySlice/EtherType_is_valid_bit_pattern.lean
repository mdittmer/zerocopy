prelude
import Init.Core
import Trusted

namespace Mir.ZerocopySlice.EtherType_is_valid_bit_pattern

open Trusted.Rustc.V2026_06_01.Layout
open Trusted.Rustc.V2026_06_01.MirAst
open Trusted.Rustc.V2026_06_01.MirSemantics
open Trusted.VerificationResult

/-! Captured `Runtime(PostCleanup)` MIR and compiler layout table for `zerocopy_slice::EtherType::is_valid_bit_pattern`. -/

-- TypeLayout[0]: size=1, align=1
-- TypeLayout[1]: size=2, align=2
def layouts : LayoutTable :=
  { pointerBits := 64, isLittleEndian := true, entries := (List.cons { size := { bytes := 1 }, align := { bytes := 1 }, backendRepr := (BackendRepr.scalar (ScalarLayout.initialized (PrimitiveKind.int 8 false) { start := 0, end_ := 1 })), fields := FieldsShape.primitive, variants := (VariantsShape.single 0), isUninhabited := false } (List.cons { size := { bytes := 2 }, align := { bytes := 2 }, backendRepr := (BackendRepr.scalar (ScalarLayout.initialized (PrimitiveKind.int 16 false) { start := 0, end_ := 65535 })), fields := FieldsShape.primitive, variants := (VariantsShape.single 0), isUninhabited := false } List.nil)) }

-- bb0: _2 = Eq(copy _1, const 2048_u16); switchInt(move _2) -> [0: bb2, otherwise: bb1]
-- bb1: _0 = const true; goto -> bb3
-- bb2: _0 = Eq(copy _1, const 34525_u16); goto -> bb3
-- bb3: return
def body : CapturedMir :=
  {
    defPath := "zerocopy_slice::EtherType::is_valid_bit_pattern",
    phase := MirPhaseTag.runtimeOptimized,
    role := ContractRole.subject,
    argCount := 1,
    localDecls := (List.cons { mutability := Mutability.mut, ty := { kind := RigidTy.bool, layoutIdx := 0 }, debugName := Option.none } (List.cons { mutability := Mutability.not, ty := { kind := (RigidTy.uint UintTy.u16), layoutIdx := 1 }, debugName := (Option.some "raw") } (List.cons { mutability := Mutability.mut, ty := { kind := RigidTy.bool, layoutIdx := 0 }, debugName := Option.none } List.nil))),
    basicBlocks := (List.cons { statements := (List.cons { kind := (StatementKind.storageLive 2), span := "<dummy>" } (List.cons { kind := (StatementKind.assign { local_ := 2, projection := List.nil } (Rvalue.binaryOp BinOp.eq (Operand.copy { local_ := 1, projection := List.nil }) (Operand.constant { ty := { kind := (RigidTy.uint UintTy.u16), layoutIdx := 1 }, val := (ConstValue.scalar (ConstScalar.int { bits := 2048, sizeBytes := 2 })) }))), span := "src/lib.rs:175:9: 175:22" } List.nil)), terminator := { kind := (TerminatorKind.switchInt (Operand.move { local_ := 2, projection := List.nil }) { branches := (List.cons (0, 2) List.nil), otherwise := 1 }), span := "src/lib.rs:175:9: 175:22" }, isCleanup := false } (List.cons { statements := (List.cons { kind := (StatementKind.assign { local_ := 0, projection := List.nil } (Rvalue.use (Operand.constant { ty := { kind := RigidTy.bool, layoutIdx := 0 }, val := (ConstValue.scalar (ConstScalar.int { bits := 1, sizeBytes := 1 })) }))), span := "src/lib.rs:175:9: 175:39" } List.nil), terminator := { kind := (TerminatorKind.goto 3), span := "src/lib.rs:175:9: 175:39" }, isCleanup := false } (List.cons { statements := (List.cons { kind := (StatementKind.assign { local_ := 0, projection := List.nil } (Rvalue.binaryOp BinOp.eq (Operand.copy { local_ := 1, projection := List.nil }) (Operand.constant { ty := { kind := (RigidTy.uint UintTy.u16), layoutIdx := 1 }, val := (ConstValue.scalar (ConstScalar.int { bits := 34525, sizeBytes := 2 })) }))), span := "src/lib.rs:175:26: 175:39" } List.nil), terminator := { kind := (TerminatorKind.goto 3), span := "src/lib.rs:175:9: 175:39" }, isCleanup := false } (List.cons { statements := List.nil, terminator := { kind := TerminatorKind.return_, span := "src/lib.rs:176:6: 176:6" }, isCleanup := false } List.nil)))),
    fnAbi := { args := (List.cons { layout := { size := { bytes := 2 }, align := { bytes := 2 }, backendRepr := (BackendRepr.scalar (ScalarLayout.initialized (PrimitiveKind.int 16 false) { start := 0, end_ := 65535 })), fields := FieldsShape.primitive, variants := (VariantsShape.single 0), isUninhabited := false }, mode := AbiPassMode.direct } List.nil), ret := { layout := { size := { bytes := 1 }, align := { bytes := 1 }, backendRepr := (BackendRepr.scalar (ScalarLayout.initialized (PrimitiveKind.int 8 false) { start := 0, end_ := 1 })), fields := FieldsShape.primitive, variants := (VariantsShape.single 0), isUninhabited := false }, mode := AbiPassMode.direct }, conv := CallConv.rust, canUnwind := true },
    layouts := layouts,
    allocations := List.nil
  }

def defaultClaim : Claim :=
  { body := body, pre := fun _ => True, post := fun _ _ => True }

end Mir.ZerocopySlice.EtherType_is_valid_bit_pattern
