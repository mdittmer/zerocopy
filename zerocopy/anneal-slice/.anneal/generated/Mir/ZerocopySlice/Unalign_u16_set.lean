prelude
import Init.Core
import Trusted

namespace Mir.ZerocopySlice.Unalign_u16_set

open Trusted.Rustc.V2026_06_01.Layout
open Trusted.Rustc.V2026_06_01.MirAst
open Trusted.Rustc.V2026_06_01.MirSemantics
open Trusted.VerificationResult

/-! Captured `Runtime(PostCleanup)` MIR and compiler layout table for `zerocopy_slice::Unalign::<u16>::set`. -/

-- TypeLayout[0]: size=2, align=2
-- TypeLayout[1]: size=8, align=8
-- TypeLayout[2]: size=2, align=1
def layouts : LayoutTable :=
  { pointerBits := 64, isLittleEndian := true, entries := (List.cons { size := { bytes := 2 }, align := { bytes := 2 }, backendRepr := (BackendRepr.scalar (ScalarLayout.initialized (PrimitiveKind.int 16 false) { start := 0, end_ := 65535 })), fields := FieldsShape.primitive, variants := (VariantsShape.single 0), isUninhabited := false } (List.cons { size := { bytes := 8 }, align := { bytes := 8 }, backendRepr := (BackendRepr.scalar (ScalarLayout.initialized (PrimitiveKind.pointer 0) { start := 1, end_ := 18446744073709551615 })), fields := FieldsShape.primitive, variants := (VariantsShape.single 0), isUninhabited := false } (List.cons { size := { bytes := 2 }, align := { bytes := 1 }, backendRepr := BackendRepr.memory, fields := (FieldsShape.arbitrary (List.cons { bytes := 0 } List.nil) (List.cons 0 List.nil)), variants := (VariantsShape.single 0), isUninhabited := false } List.nil))) }

-- bb0: _3 = Unalign::<u16>(copy _2); (*_1) = move _3; _0 = copy ((*_1).0: u16); return
def body : CapturedMir :=
  {
    defPath := "zerocopy_slice::Unalign::<u16>::set",
    phase := MirPhaseTag.runtimeOptimized,
    role := ContractRole.subject,
    argCount := 2,
    localDecls := (List.cons { mutability := Mutability.mut, ty := { kind := (RigidTy.uint UintTy.u16), layoutIdx := 0 }, debugName := Option.none } (List.cons { mutability := Mutability.not, ty := { kind := (RigidTy.ref Mutability.mut (RigidTy.adt "zerocopy_slice::Unalign" (List.cons (RigidTy.uint UintTy.u16) List.nil))), layoutIdx := 1 }, debugName := (Option.some "self") } (List.cons { mutability := Mutability.not, ty := { kind := (RigidTy.uint UintTy.u16), layoutIdx := 0 }, debugName := (Option.some "val") } (List.cons { mutability := Mutability.mut, ty := { kind := (RigidTy.adt "zerocopy_slice::Unalign" (List.cons (RigidTy.uint UintTy.u16) List.nil)), layoutIdx := 2 }, debugName := Option.none } List.nil)))),
    basicBlocks := (List.cons { statements := (List.cons { kind := (StatementKind.storageLive 3), span := "<dummy>" } (List.cons { kind := (StatementKind.assign { local_ := 3, projection := List.nil } (Rvalue.aggregate (AggregateKind.adt "zerocopy_slice::Unalign" 0 Option.none) (List.cons (Operand.copy { local_ := 2, projection := List.nil }) List.nil))), span := "src/lib.rs:101:17: 101:29" } (List.cons { kind := (StatementKind.assign { local_ := 1, projection := (List.cons ProjectionElem.deref List.nil) } (Rvalue.use (Operand.move { local_ := 3, projection := List.nil }))), span := "src/lib.rs:101:9: 101:29" } (List.cons { kind := (StatementKind.assign { local_ := 0, projection := List.nil } (Rvalue.use (Operand.copy { local_ := 1, projection := (List.cons ProjectionElem.deref (List.cons (ProjectionElem.field 0 { kind := (RigidTy.uint UintTy.u16), layoutIdx := 0 }) List.nil)) }))), span := "src/lib.rs:102:9: 102:15" } List.nil)))), terminator := { kind := TerminatorKind.return_, span := "src/lib.rs:103:6: 103:6" }, isCleanup := false } List.nil),
    fnAbi := { args := (List.cons { layout := { size := { bytes := 8 }, align := { bytes := 8 }, backendRepr := (BackendRepr.scalar (ScalarLayout.initialized (PrimitiveKind.pointer 0) { start := 1, end_ := 18446744073709551615 })), fields := FieldsShape.primitive, variants := (VariantsShape.single 0), isUninhabited := false }, mode := AbiPassMode.direct } (List.cons { layout := { size := { bytes := 2 }, align := { bytes := 2 }, backendRepr := (BackendRepr.scalar (ScalarLayout.initialized (PrimitiveKind.int 16 false) { start := 0, end_ := 65535 })), fields := FieldsShape.primitive, variants := (VariantsShape.single 0), isUninhabited := false }, mode := AbiPassMode.direct } List.nil)), ret := { layout := { size := { bytes := 2 }, align := { bytes := 2 }, backendRepr := (BackendRepr.scalar (ScalarLayout.initialized (PrimitiveKind.int 16 false) { start := 0, end_ := 65535 })), fields := FieldsShape.primitive, variants := (VariantsShape.single 0), isUninhabited := false }, mode := AbiPassMode.direct }, conv := CallConv.rust, canUnwind := true },
    layouts := layouts,
    allocations := List.nil
  }

def defaultClaim : Claim :=
  { body := body, pre := fun _ => True, post := fun _ _ => True }

end Mir.ZerocopySlice.Unalign_u16_set
