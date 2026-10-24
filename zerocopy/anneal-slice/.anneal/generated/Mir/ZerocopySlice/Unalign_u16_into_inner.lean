prelude
import Init.Core
import Trusted

namespace Mir.ZerocopySlice.Unalign_u16_into_inner

open Trusted.Rustc.V2026_06_01.Layout
open Trusted.Rustc.V2026_06_01.MirAst
open Trusted.Rustc.V2026_06_01.MirSemantics
open Trusted.VerificationResult

/-! Captured `Runtime(PostCleanup)` MIR and compiler layout table for `zerocopy_slice::Unalign::<u16>::into_inner`. -/

-- TypeLayout[0]: size=2, align=2
-- TypeLayout[1]: size=2, align=1
-- TypeLayout[2]: size=2, align=1
-- TypeLayout[3]: size=2, align=2
def layouts : LayoutTable :=
  { pointerBits := 64, isLittleEndian := true, entries := (List.cons { size := { bytes := 2 }, align := { bytes := 2 }, backendRepr := (BackendRepr.scalar (ScalarLayout.initialized (PrimitiveKind.int 16 false) { start := 0, end_ := 65535 })), fields := FieldsShape.primitive, variants := (VariantsShape.single 0), isUninhabited := false } (List.cons { size := { bytes := 2 }, align := { bytes := 1 }, backendRepr := BackendRepr.memory, fields := (FieldsShape.arbitrary (List.cons { bytes := 0 } List.nil) (List.cons 0 List.nil)), variants := (VariantsShape.single 0), isUninhabited := false } (List.cons { size := { bytes := 2 }, align := { bytes := 1 }, backendRepr := BackendRepr.memory, fields := (FieldsShape.array { bytes := 1 } 2), variants := (VariantsShape.single 0), isUninhabited := false } (List.cons { size := { bytes := 2 }, align := { bytes := 2 }, backendRepr := BackendRepr.memory, fields := (FieldsShape.union 2), variants := (VariantsShape.single 0), isUninhabited := false } List.nil)))) }

-- bb0: _2 = copy _1 as [u8; 2] (Transmute); _3 = Transmute::<[u8; 2], u16> { src: copy _2 }; _0 = copy _3 as u16 (Transmute); return
def body : CapturedMir :=
  {
    defPath := "zerocopy_slice::Unalign::<u16>::into_inner",
    phase := MirPhaseTag.runtimeOptimized,
    role := ContractRole.subject,
    argCount := 1,
    localDecls := (List.cons { mutability := Mutability.mut, ty := { kind := (RigidTy.uint UintTy.u16), layoutIdx := 0 }, debugName := Option.none } (List.cons { mutability := Mutability.not, ty := { kind := (RigidTy.adt "zerocopy_slice::Unalign" (List.cons (RigidTy.uint UintTy.u16) List.nil)), layoutIdx := 1 }, debugName := (Option.some "self") } (List.cons { mutability := Mutability.not, ty := { kind := (RigidTy.array (RigidTy.uint UintTy.u8) 2), layoutIdx := 2 }, debugName := (Option.some "bytes") } (List.cons { mutability := Mutability.not, ty := { kind := (RigidTy.adt "zerocopy_slice::Transmute" (List.cons (RigidTy.array (RigidTy.uint UintTy.u8) 2) (List.cons (RigidTy.uint UintTy.u16) List.nil))), layoutIdx := 3 }, debugName := (Option.some "u") } List.nil)))),
    basicBlocks := (List.cons { statements := (List.cons { kind := (StatementKind.storageLive 2), span := "<dummy>" } (List.cons { kind := (StatementKind.storageLive 3), span := "<dummy>" } (List.cons { kind := (StatementKind.assign { local_ := 2, projection := List.nil } (Rvalue.cast CastKind.transmute (Operand.copy { local_ := 1, projection := List.nil }) { kind := (RigidTy.array (RigidTy.uint UintTy.u8) 2), layoutIdx := 2 })), span := "src/lib.rs:75:22: 75:73" } (List.cons { kind := (StatementKind.assign { local_ := 3, projection := List.nil } (Rvalue.aggregate (AggregateKind.adt "zerocopy_slice::Transmute" 0 (Option.some 0)) (List.cons (Operand.copy { local_ := 2, projection := List.nil }) List.nil))), span := "src/lib.rs:52:17: 52:58" } (List.cons { kind := (StatementKind.assign { local_ := 0, projection := List.nil } (Rvalue.cast CastKind.transmute (Operand.copy { local_ := 3, projection := List.nil }) { kind := (RigidTy.uint UintTy.u16), layoutIdx := 0 })), span := "src/lib.rs:55:18: 55:78" } List.nil))))), terminator := { kind := TerminatorKind.return_, span := "src/lib.rs:77:6: 77:6" }, isCleanup := false } List.nil),
    fnAbi := { args := (List.cons { layout := { size := { bytes := 2 }, align := { bytes := 1 }, backendRepr := BackendRepr.memory, fields := (FieldsShape.arbitrary (List.cons { bytes := 0 } List.nil) (List.cons 0 List.nil)), variants := (VariantsShape.single 0), isUninhabited := false }, mode := AbiPassMode.direct } List.nil), ret := { layout := { size := { bytes := 2 }, align := { bytes := 2 }, backendRepr := (BackendRepr.scalar (ScalarLayout.initialized (PrimitiveKind.int 16 false) { start := 0, end_ := 65535 })), fields := FieldsShape.primitive, variants := (VariantsShape.single 0), isUninhabited := false }, mode := AbiPassMode.direct }, conv := CallConv.rust, canUnwind := true },
    layouts := layouts,
    allocations := List.nil
  }

def defaultClaim : Claim :=
  { body := body, pre := fun _ => True, post := fun _ _ => True }

end Mir.ZerocopySlice.Unalign_u16_into_inner
