prelude
import Init.Core
import Trusted

namespace Mir.ZerocopySlice.transmute_unchecked

open Trusted.Rustc.V2026_06_01.Layout
open Trusted.Rustc.V2026_06_01.MirAst
open Trusted.Rustc.V2026_06_01.MirSemantics
open Trusted.VerificationResult

/-! Captured `Runtime(PostCleanup)` MIR and compiler layout table for `zerocopy_slice::transmute_unchecked`. -/

-- TypeLayout[0]: size=2, align=1
-- TypeLayout[1]: size=2, align=2
-- TypeLayout[2]: size=2, align=2
def layouts : LayoutTable :=
  { pointerBits := 64, isLittleEndian := true, entries := (List.cons { size := { bytes := 2 }, align := { bytes := 1 }, backendRepr := BackendRepr.memory, fields := (FieldsShape.array { bytes := 1 } 2), variants := (VariantsShape.single 0), isUninhabited := false } (List.cons { size := { bytes := 2 }, align := { bytes := 2 }, backendRepr := (BackendRepr.scalar (ScalarLayout.initialized (PrimitiveKind.int 16 false) { start := 0, end_ := 65535 })), fields := FieldsShape.primitive, variants := (VariantsShape.single 0), isUninhabited := false } (List.cons { size := { bytes := 2 }, align := { bytes := 2 }, backendRepr := BackendRepr.memory, fields := (FieldsShape.union 2), variants := (VariantsShape.single 0), isUninhabited := false } List.nil))) }

-- bb0: _2 = Transmute::<u16, [u8; 2]> { src: copy _1 }; _0 = copy _2 as [u8; 2] (Transmute); return
def body : CapturedMir :=
  {
    defPath := "zerocopy_slice::transmute_unchecked",
    phase := MirPhaseTag.runtimeOptimized,
    role := ContractRole.subject,
    argCount := 1,
    localDecls := (List.cons { mutability := Mutability.mut, ty := { kind := (RigidTy.array (RigidTy.uint UintTy.u8) 2), layoutIdx := 0 }, debugName := Option.none } (List.cons { mutability := Mutability.not, ty := { kind := (RigidTy.uint UintTy.u16), layoutIdx := 1 }, debugName := (Option.some "src") } (List.cons { mutability := Mutability.not, ty := { kind := (RigidTy.adt "zerocopy_slice::Transmute" (List.cons (RigidTy.uint UintTy.u16) (List.cons (RigidTy.array (RigidTy.uint UintTy.u8) 2) List.nil))), layoutIdx := 2 }, debugName := (Option.some "u") } List.nil))),
    basicBlocks := (List.cons { statements := (List.cons { kind := (StatementKind.storageLive 2), span := "<dummy>" } (List.cons { kind := (StatementKind.assign { local_ := 2, projection := List.nil } (Rvalue.aggregate (AggregateKind.adt "zerocopy_slice::Transmute" 0 (Option.some 0)) (List.cons (Operand.copy { local_ := 1, projection := List.nil }) List.nil))), span := "src/lib.rs:52:17: 52:58" } (List.cons { kind := (StatementKind.assign { local_ := 0, projection := List.nil } (Rvalue.cast CastKind.transmute (Operand.copy { local_ := 2, projection := List.nil }) { kind := (RigidTy.array (RigidTy.uint UintTy.u8) 2), layoutIdx := 0 })), span := "src/lib.rs:55:18: 55:78" } List.nil))), terminator := { kind := TerminatorKind.return_, span := "src/lib.rs:111:2: 111:2" }, isCleanup := false } List.nil),
    fnAbi := { args := (List.cons { layout := { size := { bytes := 2 }, align := { bytes := 2 }, backendRepr := (BackendRepr.scalar (ScalarLayout.initialized (PrimitiveKind.int 16 false) { start := 0, end_ := 65535 })), fields := FieldsShape.primitive, variants := (VariantsShape.single 0), isUninhabited := false }, mode := AbiPassMode.direct } List.nil), ret := { layout := { size := { bytes := 2 }, align := { bytes := 1 }, backendRepr := BackendRepr.memory, fields := (FieldsShape.array { bytes := 1 } 2), variants := (VariantsShape.single 0), isUninhabited := false }, mode := AbiPassMode.direct }, conv := CallConv.rust, canUnwind := true },
    layouts := layouts,
    allocations := List.nil
  }

def defaultClaim : Claim :=
  { body := body, pre := fun _ => True, post := fun _ _ => True }

end Mir.ZerocopySlice.transmute_unchecked
