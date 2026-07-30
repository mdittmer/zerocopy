/-
Copyright 2026 The Fuchsia Authors

Licensed under the 2-Clause BSD License <LICENSE-BSD or
https://opensource.org/license/bsd-2-clause>, Apache License, Version 2.0
<LICENSE-APACHE or https://www.apache.org/licenses/LICENSE-2.0>, or the MIT
license <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your option.
This file may not be copied, modified, or distributed except according to
those terms.
-/

import Lean
import Lean.Util.CollectAxioms

open Lean

private def joinedNames (names : Array Name) : String :=
  String.intercalate ", " (names.toList.map toString)

private unsafe def auditModule (moduleName : Name) (allowSorry : Bool) : IO Unit := do
  Lean.withImportModules #[{ module := moduleName }] {} fun env => do
    let some moduleIdx := env.getModuleIdx? moduleName
      | throw <| IO.userError s!"Anneal axiom audit could not find module '{moduleName}'"

    let mut localDecls := #[]
    let mut localAxioms := #[]
    for (name, info) in env.constants do
      if env.getModuleIdxFor? name == some moduleIdx then
        localDecls := localDecls.push name
        if info matches .axiomInfo _ then
          localAxioms := localAxioms.push name

    unless localAxioms.isEmpty do
      throw <| IO.userError s!"Anneal rejected axiom declaration(s) added while elaborating \
        '{moduleName}': {joinedNames localAxioms}"

    unless allowSorry do
      let context : Core.Context := {
        fileName := "<anneal-axiom-audit>"
        fileMap := FileMap.ofString ""
      }
      let state : Core.State := { env }
      for name in localDecls do
        let axioms ← Core.CoreM.toIO' (Lean.collectAxioms name) context state
        if axioms.contains ``sorryAx then
          throw <| IO.userError s!"Anneal rejected declaration '{name}' because it depends on \
            'sorryAx'; pass '--allow-sorry' explicitly to permit proof admissions"

unsafe def main (args : List String) : IO UInt32 := do
  initSearchPath (← findSysroot)
  let allowSorry := args.contains "--allow-sorry"
  let modules := args.filter (· != "--allow-sorry")
  if modules.isEmpty then
    throw <| IO.userError "usage: AnnealAxiomAudit [--allow-sorry] MODULE..."
  for moduleArg in modules do
    if moduleArg.startsWith "-" then
      throw <| IO.userError s!"unknown Anneal axiom-audit option: {moduleArg}"
    let moduleName := moduleArg.toName
    if moduleName.isAnonymous then
      throw <| IO.userError s!"invalid Lean module name: {moduleArg}"
    auditModule moduleName allowSorry
  return 0
