prelude
import Init.Core
import Trusted

namespace Mir.ZerocopySlice.PacketHeader_total_packet_len

open Trusted.Rustc.V2026_06_01.Layout
open Trusted.Rustc.V2026_06_01.MirAst
open Trusted.Rustc.V2026_06_01.MirSemantics
open Trusted.VerificationResult

/-! Captured `Runtime(PostCleanup)` MIR and compiler layout table for `zerocopy_slice::PacketHeader::total_packet_len`. -/

-- TypeLayout[0]: size=2, align=2
-- TypeLayout[1]: size=6, align=1
-- TypeLayout[2]: size=8, align=8
-- TypeLayout[3]: size=4, align=2
-- TypeLayout[4]: size=2, align=1
-- TypeLayout[5]: size=1, align=1
def layouts : LayoutTable :=
  { pointerBits := 64, isLittleEndian := true, entries := (List.cons { size := { bytes := 2 }, align := { bytes := 2 }, backendRepr := (BackendRepr.scalar (ScalarLayout.initialized (PrimitiveKind.int 16 false) { start := 0, end_ := 65535 })), fields := FieldsShape.primitive, variants := (VariantsShape.single 0), isUninhabited := false } (List.cons { size := { bytes := 6 }, align := { bytes := 1 }, backendRepr := BackendRepr.memory, fields := (FieldsShape.arbitrary (List.cons { bytes := 0 } (List.cons { bytes := 2 } (List.cons { bytes := 4 } List.nil))) (List.cons 0 (List.cons 1 (List.cons 2 List.nil)))), variants := (VariantsShape.single 0), isUninhabited := false } (List.cons { size := { bytes := 8 }, align := { bytes := 8 }, backendRepr := (BackendRepr.scalar (ScalarLayout.initialized (PrimitiveKind.pointer 0) { start := 1, end_ := 18446744073709551615 })), fields := FieldsShape.primitive, variants := (VariantsShape.single 0), isUninhabited := false } (List.cons { size := { bytes := 4 }, align := { bytes := 2 }, backendRepr := (BackendRepr.scalarPair (ScalarLayout.initialized (PrimitiveKind.int 16 false) { start := 0, end_ := 65535 }) (ScalarLayout.initialized (PrimitiveKind.int 8 false) { start := 0, end_ := 1 })), fields := (FieldsShape.arbitrary (List.cons { bytes := 0 } (List.cons { bytes := 2 } List.nil)) (List.cons 0 (List.cons 1 List.nil))), variants := (VariantsShape.single 0), isUninhabited := false } (List.cons { size := { bytes := 2 }, align := { bytes := 1 }, backendRepr := BackendRepr.memory, fields := (FieldsShape.arbitrary (List.cons { bytes := 0 } List.nil) (List.cons 0 List.nil)), variants := (VariantsShape.single 0), isUninhabited := false } (List.cons { size := { bytes := 1 }, align := { bytes := 1 }, backendRepr := (BackendRepr.scalar (ScalarLayout.initialized (PrimitiveKind.int 8 false) { start := 0, end_ := 1 })), fields := FieldsShape.primitive, variants := (VariantsShape.single 0), isUninhabited := false } List.nil)))))) }

-- bb0: _2 = &(_1.2: Unalign<u16>); _3 = copy ((*_2).0: u16); _4 = AddWithOverflow(const 6_u16, copy _3); assert(!move (_4.1: bool), "attempt to compute `{} + {}`, which would overflow", const 6_u16, copy _3) -> [success: bb1, unwind continue]
-- bb1: _0 = move (_4.0: u16); return
def body : CapturedMir :=
  {
    defPath := "zerocopy_slice::PacketHeader::total_packet_len",
    phase := MirPhaseTag.runtimeOptimized,
    role := ContractRole.subject,
    argCount := 1,
    localDecls := (List.cons { mutability := Mutability.mut, ty := { kind := (RigidTy.uint UintTy.u16), layoutIdx := 0 }, debugName := Option.none } (List.cons { mutability := Mutability.not, ty := { kind := (RigidTy.adt "zerocopy_slice::PacketHeader" List.nil), layoutIdx := 1 }, debugName := (Option.some "self") } (List.cons { mutability := Mutability.not, ty := { kind := (RigidTy.ref Mutability.not (RigidTy.adt "zerocopy_slice::Unalign" (List.cons (RigidTy.uint UintTy.u16) List.nil))), layoutIdx := 2 }, debugName := (Option.some "payload_ref") } (List.cons { mutability := Mutability.not, ty := { kind := (RigidTy.uint UintTy.u16), layoutIdx := 0 }, debugName := (Option.some "payload") } (List.cons { mutability := Mutability.mut, ty := { kind := (RigidTy.tuple (List.cons (RigidTy.uint UintTy.u16) (List.cons RigidTy.bool List.nil))), layoutIdx := 3 }, debugName := Option.none } List.nil))))),
    basicBlocks := (List.cons { statements := (List.cons { kind := (StatementKind.storageLive 2), span := "<dummy>" } (List.cons { kind := (StatementKind.storageLive 3), span := "<dummy>" } (List.cons { kind := (StatementKind.storageLive 4), span := "<dummy>" } (List.cons { kind := (StatementKind.assign { local_ := 2, projection := List.nil } (Rvalue.ref BorrowKind.shared { local_ := 1, projection := (List.cons (ProjectionElem.field 2 { kind := (RigidTy.adt "zerocopy_slice::Unalign" (List.cons (RigidTy.uint UintTy.u16) List.nil)), layoutIdx := 4 }) List.nil) })), span := "src/lib.rs:174:42: 174:59" } (List.cons { kind := (StatementKind.assign { local_ := 3, projection := List.nil } (Rvalue.use (Operand.copy { local_ := 2, projection := (List.cons ProjectionElem.deref (List.cons (ProjectionElem.field 0 { kind := (RigidTy.uint UintTy.u16), layoutIdx := 0 }) List.nil)) }))), span := "src/lib.rs:175:28: 175:41" } (List.cons { kind := (StatementKind.assign { local_ := 4, projection := List.nil } (Rvalue.binaryOp BinOp.addWithOverflow (Operand.constant { ty := { kind := (RigidTy.uint UintTy.u16), layoutIdx := 0 }, val := (ConstValue.scalar (ConstScalar.int { bits := 6, sizeBytes := 2 })) }) (Operand.copy { local_ := 3, projection := List.nil }))), span := "src/lib.rs:176:9: 176:23" } List.nil)))))), terminator := { kind := (TerminatorKind.assert (Operand.move { local_ := 4, projection := (List.cons (ProjectionElem.field 1 { kind := RigidTy.bool, layoutIdx := 5 }) List.nil) }) false 1 UnwindAction.continue_), span := "src/lib.rs:176:9: 176:23" }, isCleanup := false } (List.cons { statements := (List.cons { kind := (StatementKind.assign { local_ := 0, projection := List.nil } (Rvalue.use (Operand.move { local_ := 4, projection := (List.cons (ProjectionElem.field 0 { kind := (RigidTy.uint UintTy.u16), layoutIdx := 0 }) List.nil) }))), span := "src/lib.rs:176:9: 176:23" } List.nil), terminator := { kind := TerminatorKind.return_, span := "src/lib.rs:177:6: 177:6" }, isCleanup := false } List.nil)),
    fnAbi := { args := (List.cons { layout := { size := { bytes := 6 }, align := { bytes := 1 }, backendRepr := BackendRepr.memory, fields := (FieldsShape.arbitrary (List.cons { bytes := 0 } (List.cons { bytes := 2 } (List.cons { bytes := 4 } List.nil))) (List.cons 0 (List.cons 1 (List.cons 2 List.nil)))), variants := (VariantsShape.single 0), isUninhabited := false }, mode := AbiPassMode.direct } List.nil), ret := { layout := { size := { bytes := 2 }, align := { bytes := 2 }, backendRepr := (BackendRepr.scalar (ScalarLayout.initialized (PrimitiveKind.int 16 false) { start := 0, end_ := 65535 })), fields := FieldsShape.primitive, variants := (VariantsShape.single 0), isUninhabited := false }, mode := AbiPassMode.direct }, conv := CallConv.rust, canUnwind := true },
    layouts := layouts,
    allocations := List.nil
  }

def defaultClaim : Claim :=
  { body := body, pre := fun _ => True, post := fun _ _ => True }

end Mir.ZerocopySlice.PacketHeader_total_packet_len
