prelude
import Init.Core
import Trusted

namespace Mir.ZerocopySlice.Ptr_a_PacketHeader_as_ref

open Trusted.Rustc.V2026_06_01.Layout
open Trusted.Rustc.V2026_06_01.MirAst
open Trusted.Rustc.V2026_06_01.MirSemantics
open Trusted.VerificationResult

/-! Captured `Runtime(PostCleanup)` MIR and compiler layout table for `zerocopy_slice::Ptr::<'a, PacketHeader>::as_ref`. -/

-- TypeLayout[0]: size=8, align=8
-- TypeLayout[1]: size=8, align=8
-- TypeLayout[2]: size=8, align=8
def layouts : LayoutTable :=
  { pointerBits := 64, isLittleEndian := true, entries := (List.cons { size := { bytes := 8 }, align := { bytes := 8 }, backendRepr := (BackendRepr.scalar (ScalarLayout.initialized (PrimitiveKind.pointer 0) { start := 1, end_ := 18446744073709551615 })), fields := FieldsShape.primitive, variants := (VariantsShape.single 0), isUninhabited := false } (List.cons { size := { bytes := 8 }, align := { bytes := 8 }, backendRepr := (BackendRepr.scalar (ScalarLayout.initialized (PrimitiveKind.pointer 0) { start := 0, end_ := 18446744073709551615 })), fields := (FieldsShape.arbitrary (List.cons { bytes := 0 } (List.cons { bytes := 8 } List.nil)) (List.cons 0 (List.cons 1 List.nil))), variants := (VariantsShape.single 0), isUninhabited := false } (List.cons { size := { bytes := 8 }, align := { bytes := 8 }, backendRepr := (BackendRepr.scalar (ScalarLayout.initialized (PrimitiveKind.pointer 0) { start := 0, end_ := 18446744073709551615 })), fields := FieldsShape.primitive, variants := (VariantsShape.single 0), isUninhabited := false } List.nil))) }

-- bb0: _2 = copy (_1.0: *const PacketHeader); _0 = copy _2 as &PacketHeader (Transmute); return
def body : CapturedMir :=
  {
    defPath := "zerocopy_slice::Ptr::<'a, PacketHeader>::as_ref",
    phase := MirPhaseTag.runtimeOptimized,
    role := ContractRole.subject,
    argCount := 1,
    localDecls := (List.cons { mutability := Mutability.mut, ty := { kind := (RigidTy.ref Mutability.not (RigidTy.adt "zerocopy_slice::PacketHeader" List.nil)), layoutIdx := 0 }, debugName := Option.none } (List.cons { mutability := Mutability.not, ty := { kind := (RigidTy.adt "zerocopy_slice::Ptr" (List.cons (RigidTy.adt "zerocopy_slice::PacketHeader" List.nil) List.nil)), layoutIdx := 1 }, debugName := (Option.some "self") } (List.cons { mutability := Mutability.not, ty := { kind := (RigidTy.rawPtr Mutability.not (RigidTy.adt "zerocopy_slice::PacketHeader" List.nil)), layoutIdx := 2 }, debugName := (Option.some "raw") } List.nil))),
    basicBlocks := (List.cons { statements := (List.cons { kind := (StatementKind.storageLive 2), span := "<dummy>" } (List.cons { kind := (StatementKind.assign { local_ := 2, projection := List.nil } (Rvalue.use (Operand.copy { local_ := 1, projection := (List.cons (ProjectionElem.field 0 { kind := (RigidTy.rawPtr Mutability.not (RigidTy.adt "zerocopy_slice::PacketHeader" List.nil)), layoutIdx := 2 }) List.nil) }))), span := "src/lib.rs:217:40: 217:48" } (List.cons { kind := (StatementKind.assign { local_ := 0, projection := List.nil } (Rvalue.cast CastKind.transmute (Operand.copy { local_ := 2, projection := List.nil }) { kind := (RigidTy.ref Mutability.not (RigidTy.adt "zerocopy_slice::PacketHeader" List.nil)), layoutIdx := 0 })), span := "src/lib.rs:221:13: 221:79" } List.nil))), terminator := { kind := TerminatorKind.return_, span := "src/lib.rs:223:6: 223:6" }, isCleanup := false } List.nil),
    fnAbi := { args := (List.cons { layout := { size := { bytes := 8 }, align := { bytes := 8 }, backendRepr := (BackendRepr.scalar (ScalarLayout.initialized (PrimitiveKind.pointer 0) { start := 0, end_ := 18446744073709551615 })), fields := (FieldsShape.arbitrary (List.cons { bytes := 0 } (List.cons { bytes := 8 } List.nil)) (List.cons 0 (List.cons 1 List.nil))), variants := (VariantsShape.single 0), isUninhabited := false }, mode := AbiPassMode.direct } List.nil), ret := { layout := { size := { bytes := 8 }, align := { bytes := 8 }, backendRepr := (BackendRepr.scalar (ScalarLayout.initialized (PrimitiveKind.pointer 0) { start := 1, end_ := 18446744073709551615 })), fields := FieldsShape.primitive, variants := (VariantsShape.single 0), isUninhabited := false }, mode := AbiPassMode.direct }, conv := CallConv.rust, canUnwind := true },
    layouts := layouts,
    allocations := List.nil
  }

def defaultClaim : Claim :=
  { body := body, pre := fun _ => True, post := fun _ _ => True }

end Mir.ZerocopySlice.Ptr_a_PacketHeader_as_ref
