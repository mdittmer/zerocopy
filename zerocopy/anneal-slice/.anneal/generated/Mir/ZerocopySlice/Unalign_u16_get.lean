prelude
import Init.Core
import Trusted

namespace Mir.ZerocopySlice.Unalign_u16_get

open Trusted.Rustc.V2026_06_01.Layout
open Trusted.Rustc.V2026_06_01.MirAst
open Trusted.Rustc.V2026_06_01.MirSemantics
open Trusted.VerificationResult

/-! Captured `Runtime(PostCleanup)` MIR and compiler layout table for `zerocopy_slice::Unalign::<u16>::get`. -/

-- TypeLayout[0]: size=2, align=2
-- TypeLayout[1]: size=8, align=8
-- TypeLayout[2]: size=2, align=1
def layouts : LayoutTable :=
  { pointerBits := 64, isLittleEndian := true, entries := (List.cons { size := { bytes := 2 }, align := { bytes := 2 }, backendRepr := (BackendRepr.scalar (ScalarLayout.initialized (PrimitiveKind.int 16 false) { start := 0, end_ := 65535 })), fields := FieldsShape.primitive, variants := (VariantsShape.single 0), isUninhabited := false } (List.cons { size := { bytes := 8 }, align := { bytes := 8 }, backendRepr := (BackendRepr.scalar (ScalarLayout.initialized (PrimitiveKind.pointer 0) { start := 1, end_ := 18446744073709551615 })), fields := FieldsShape.primitive, variants := (VariantsShape.single 0), isUninhabited := false } (List.cons { size := { bytes := 2 }, align := { bytes := 1 }, backendRepr := BackendRepr.memory, fields := (FieldsShape.arbitrary (List.cons { bytes := 0 } List.nil) (List.cons 0 List.nil)), variants := (VariantsShape.single 0), isUninhabited := false } List.nil))) }

-- bb0: _2 = Unalign::<u16>(const 4660_u16); _3 = &_2; _0 = copy ((*_1).0: u16); return
def body : CapturedMir :=
  {
    defPath := "zerocopy_slice::Unalign::<u16>::get",
    phase := MirPhaseTag.runtimeOptimized,
    role := ContractRole.subject,
    argCount := 1,
    localDecls := (List.cons { mutability := Mutability.mut, ty := { kind := (RigidTy.uint UintTy.u16), layoutIdx := 0 }, debugName := (Option.some "val") } (List.cons { mutability := Mutability.not, ty := { kind := (RigidTy.ref Mutability.not (RigidTy.adt "zerocopy_slice::Unalign" (List.cons (RigidTy.uint UintTy.u16) List.nil))), layoutIdx := 1 }, debugName := (Option.some "self") } (List.cons { mutability := Mutability.not, ty := { kind := (RigidTy.adt "zerocopy_slice::Unalign" (List.cons (RigidTy.uint UintTy.u16) List.nil)), layoutIdx := 2 }, debugName := (Option.some "local_copy") } (List.cons { mutability := Mutability.not, ty := { kind := (RigidTy.ref Mutability.not (RigidTy.adt "zerocopy_slice::Unalign" (List.cons (RigidTy.uint UintTy.u16) List.nil))), layoutIdx := 1 }, debugName := (Option.some "_borrow") } List.nil)))),
    basicBlocks := (List.cons { statements := (List.cons { kind := (StatementKind.storageLive 2), span := "<dummy>" } (List.cons { kind := (StatementKind.storageLive 3), span := "<dummy>" } (List.cons { kind := (StatementKind.assign { local_ := 2, projection := List.nil } (Rvalue.aggregate (AggregateKind.adt "zerocopy_slice::Unalign" 0 Option.none) (List.cons (Operand.constant { ty := { kind := (RigidTy.uint UintTy.u16), layoutIdx := 0 }, val := (ConstValue.scalar (ConstScalar.int { bits := 4660, sizeBytes := 2 })) }) List.nil))), span := "src/lib.rs:82:26: 82:44" } (List.cons { kind := (StatementKind.assign { local_ := 3, projection := List.nil } (Rvalue.ref BorrowKind.shared { local_ := 2, projection := List.nil })), span := "src/lib.rs:83:38: 83:49" } (List.cons { kind := (StatementKind.assign { local_ := 0, projection := List.nil } (Rvalue.use (Operand.copy { local_ := 1, projection := (List.cons ProjectionElem.deref (List.cons (ProjectionElem.field 0 { kind := (RigidTy.uint UintTy.u16), layoutIdx := 0 }) List.nil)) }))), span := "src/lib.rs:84:21: 84:24" } List.nil))))), terminator := { kind := TerminatorKind.return_, span := "src/lib.rs:86:6: 86:6" }, isCleanup := false } List.nil),
    fnAbi := { args := (List.cons { layout := { size := { bytes := 8 }, align := { bytes := 8 }, backendRepr := (BackendRepr.scalar (ScalarLayout.initialized (PrimitiveKind.pointer 0) { start := 1, end_ := 18446744073709551615 })), fields := FieldsShape.primitive, variants := (VariantsShape.single 0), isUninhabited := false }, mode := AbiPassMode.direct } List.nil), ret := { layout := { size := { bytes := 2 }, align := { bytes := 2 }, backendRepr := (BackendRepr.scalar (ScalarLayout.initialized (PrimitiveKind.int 16 false) { start := 0, end_ := 65535 })), fields := FieldsShape.primitive, variants := (VariantsShape.single 0), isUninhabited := false }, mode := AbiPassMode.direct }, conv := CallConv.rust, canUnwind := true },
    layouts := layouts,
    allocations := List.nil
  }

def defaultClaim : Claim :=
  { body := body, pre := fun _ => True, post := fun _ _ => True }

end Mir.ZerocopySlice.Unalign_u16_get
