// Copyright 2026 The Fuchsia Authors
//
// Licensed under the 2-Clause BSD License <LICENSE-BSD or
// https://opensource.org/license/bsd-2-clause>, Apache License, Version 2.0
// <LICENSE-APACHE or https://www.apache.org/licenses/LICENSE-2.0>, or the MIT
// license <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to
// those terms.

//! Generated Lean soundness scaffolding.
//!
//! This module emits one `@[step]` theorem per supported Aeneas-generated
//! `Result` function. Unannotated functions get the default successful-return
//! policy, while Anneal annotations refine the ordinary Aeneas step theorem with
//! additional hypothesis arguments and successful-result postconditions.

use anyhow::Context as _;
use serde::Deserialize;

use crate::annotations::{
    LeanArg, LeanArgs, LeanAttr, LeanAttrs, LeanProof, LeanProp, Proof, Prop, RustConditionOwner,
    RustConditionRole, TheoremDefinition,
};

const ANNEAL_HYPOTHESIS_SEPARATOR: &str =
    "/- -------------------- Anneal hypotheses -------------------- -/";

/// Emits soundness theorems for Aeneas-generated functions.
pub fn write_soundness(
    roots: &crate::resolve::LockedRoots,
    packages: &[crate::scanner::AnnealArtifact],
    annotations: &crate::annotation_scan::AnnotationScan,
    allow_sorry: bool,
) -> anyhow::Result<()> {
    let mut matched_specs = std::collections::BTreeSet::new();

    for artifact in packages {
        let artifact_dir = artifact.lean_dir_path(roots);
        if !artifact_dir.exists() {
            continue;
        }
        let llbc_path = artifact.llbc_path(roots);
        if !llbc_path.exists() {
            continue;
        }
        let model = load_soundness_model(&artifact_dir, &llbc_path)?;
        let artifact_specs = annotations
            .function_specs()
            .iter()
            .enumerate()
            .filter(|(_, spec)| spec.package_manifest_path == artifact.manifest_path)
            .collect::<Vec<_>>();
        let artifact_theorems = annotations
            .aux_theorems()
            .iter()
            .filter(|theorem| theorem.package_manifest_path == artifact.manifest_path)
            .collect::<Vec<_>>();
        let artifact_struct_invariants = annotations
            .struct_invariants()
            .iter()
            .enumerate()
            .filter(|(_, invariant)| invariant.package_manifest_path == artifact.manifest_path)
            .collect::<Vec<_>>();

        let Some(generated) = soundness_module(
            &artifact.artifact_slug(),
            &model,
            &artifact_specs,
            &artifact_struct_invariants,
            &artifact_theorems,
            allow_sorry,
        )?
        else {
            continue;
        };
        matched_specs.extend(generated.matched_specs);
        let soundness_path = artifact_dir.join("Soundness.lean");
        std::fs::write(&soundness_path, generated.lean)
            .with_context(|| format!("Failed to write {}", soundness_path.display()))?;
    }

    let unmatched = annotations
        .function_specs()
        .iter()
        .enumerate()
        .filter(|(idx, _)| !matched_specs.contains(idx))
        .map(|(_, spec)| format!("{} `{}`", spec.path.display(), spec.rust_name))
        .collect::<Vec<_>>();
    if !unmatched.is_empty() {
        anyhow::bail!(
            "could not match {} Anneal function spec(s) to generated Aeneas definitions:\n{}",
            unmatched.len(),
            unmatched.join("\n")
        );
    }

    Ok(())
}

#[derive(Debug)]
struct GeneratedSoundness {
    lean: String,
    matched_specs: Vec<usize>,
}

#[derive(Debug, PartialEq, Eq)]
struct SoundnessModel {
    namespace: String,
    defs: Vec<ResultDef>,
    functions: Vec<TranslatedFunction>,
    types: Vec<TypeDef>,
    skipped: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct TranslatedFunction {
    def_id: usize,
    rust_name: String,
    name: String,
    direct_calls: Vec<usize>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct TypeDef {
    def_id: usize,
    rust_name: String,
    name: String,
}

impl TypeDef {
    fn rust_basename(&self) -> &str {
        rust_basename(&self.rust_name)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ResultDef {
    def_id: usize,
    rust_name: String,
    name: String,
    binders: Vec<String>,
    args: Vec<String>,
    output_type: String,
    required_structs: Vec<StructPropUse>,
    ensured_structs: Vec<StructPropUse>,
    unsupported_structs: Vec<UnsupportedStructUse>,
}

impl ResultDef {
    fn application(&self) -> String {
        if self.args.is_empty() {
            self.name.clone()
        } else {
            format!("{} {}", self.name, self.args.join(" "))
        }
    }

    fn rust_basename(&self) -> &str {
        rust_basename(&self.rust_name)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct StructPropUse {
    type_id: usize,
    value: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct UnsupportedStructUse {
    type_id: usize,
    description: String,
}

#[derive(Clone, Debug)]
struct ResolvedSpec {
    rust_name: String,
    proof: Option<Proof>,
    rust_theorem: bool,
    requires: Vec<ResolvedCondition>,
    ensures: Vec<ResolvedCondition>,
    invariants: Vec<ResolvedCondition>,
}

impl ResolvedSpec {
    fn empty(rust_name: String) -> Self {
        Self {
            rust_name,
            proof: None,
            rust_theorem: false,
            requires: Vec::new(),
            ensures: Vec::new(),
            invariants: Vec::new(),
        }
    }

    fn is_empty(&self) -> bool {
        !self.rust_theorem
            && self.proof.is_none()
            && self.requires.is_empty()
            && self.ensures.is_empty()
            && self.invariants.is_empty()
    }

    fn has_explicit_lean_proof(&self) -> bool {
        matches!(self.proof, Some(Proof::Lean(_)))
    }

    fn conditions(&self) -> impl Iterator<Item = &ResolvedCondition> {
        self.requires.iter().chain(&self.invariants).chain(&self.ensures)
    }
}

#[derive(Clone, Debug)]
enum ResolvedCondition {
    Lean(LeanProp),
    Rust {
        explicit_name: Option<String>,
        helper_def_id: usize,
        helper_lean_name: String,
        args: Vec<String>,
    },
}

impl ResolvedCondition {
    fn explicit_name(&self) -> Option<&str> {
        match self {
            Self::Lean(lean) => lean.name.as_deref(),
            Self::Rust { explicit_name, .. } => explicit_name.as_deref(),
        }
    }

    fn helper_def_id(&self) -> Option<usize> {
        match self {
            Self::Lean(_) => None,
            Self::Rust { helper_def_id, .. } => Some(*helper_def_id),
        }
    }
}

fn load_soundness_model(
    artifact_dir: &std::path::Path,
    llbc_path: &std::path::Path,
) -> anyhow::Result<SoundnessModel> {
    let llbc = llbc_crate_info(llbc_path)?;
    let translation_path = artifact_dir.join("translation.json");
    let translation: AeneasTranslation = serde_json::from_reader(
        std::fs::File::open(&translation_path)
            .with_context(|| format!("Failed to open {}", translation_path.display()))?,
    )
    .with_context(|| format!("Failed to parse {}", translation_path.display()))?;
    model_from_translation_json(&translation, &llbc)
}

fn soundness_module(
    module_name: &str,
    model: &SoundnessModel,
    specs: &[(usize, &crate::annotation_scan::FunctionSpec)],
    struct_invariants: &[(usize, &crate::annotation_scan::StructInvariant)],
    aux_theorems: &[&crate::annotation_scan::AuxTheorem],
    allow_sorry: bool,
) -> anyhow::Result<Option<GeneratedSoundness>> {
    if model.defs.is_empty()
        && specs.is_empty()
        && struct_invariants.is_empty()
        && aux_theorems.is_empty()
    {
        return Ok(None);
    }

    let mut out = String::new();
    out.push_str("-- THIS FILE WAS AUTOMATICALLY GENERATED BY ANNEAL\n");
    out.push_str("-- Ordinary Aeneas step soundness theorem shells.\n");
    out.push_str("-- Function names come from Aeneas translation.json; binders come from LLBC.\n");
    out.push_str(&format!("import {module_name}.Funs\n\n"));
    out.push_str("open Aeneas Aeneas.Std Result ControlFlow Error\n");
    if !allow_sorry {
        // Lean reports every declaration whose value depends on `sorryAx` via
        // the "declaration uses sorry" warning.  Making warnings fatal is the
        // semantic backstop for syntax spellings that a lexical pre-scan
        // cannot anticipate.
        out.push_str("set_option warningAsError true\n");
    }
    out.push_str("set_option linter.unusedVariables false\n\n");
    out.push_str("set_option linter.unusedSimpArgs false\n\n");
    out.push_str("set_option linter.unusedTactic false\n");
    out.push_str("set_option linter.unreachableTactic false\n\n");
    out.push_str(&format!("namespace {}\n\n", model.namespace));

    for skipped in &model.skipped {
        out.push_str("-- Skipped soundness scaffold: ");
        out.push_str(skipped);
        out.push('\n');
    }
    if !model.skipped.is_empty() {
        out.push('\n');
    }

    for aux in aux_theorems {
        out.push_str(&aux_theorem(aux)?);
        out.push('\n');
    }

    let struct_invariants_by_type = match_struct_invariants(model, struct_invariants)?;
    let mut matched_specs = Vec::new();
    let mut combined_specs = std::collections::BTreeMap::new();
    for (spec_idx, spec) in specs {
        let matches = model
            .defs
            .iter()
            .enumerate()
            .filter(|(_, def)| def_matches_spec(def, spec))
            .collect::<Vec<_>>();
        match matches.as_slice() {
            [] => {}
            [(def_idx, def)] => {
                matched_specs.push(*spec_idx);
                combined_specs.insert(*def_idx, resolve_function_spec(model, def, spec)?);
            }
            _ => {
                anyhow::bail!(
                    "Anneal spec for `{}` in {} is ambiguous across generated Aeneas definitions: {}",
                    spec.rust_name,
                    spec.path.display(),
                    matches.iter().map(|(_, def)| def.name.as_str()).collect::<Vec<_>>().join(", ")
                );
            }
        }
    }

    let rust_condition_helpers =
        referenced_rust_condition_helpers(model, &combined_specs, struct_invariants)?;
    apply_struct_invariants(
        model,
        &struct_invariants_by_type,
        &rust_condition_helpers,
        &mut combined_specs,
    )?;
    combined_specs.retain(|_, spec| !spec.is_empty());

    let all_condition_simp_names = combined_specs
        .iter()
        .flat_map(|(def_idx, spec)| condition_simp_names(&model.defs[*def_idx], spec))
        .collect::<Vec<_>>();

    for (def_idx, def) in model.defs.iter().enumerate() {
        if let Some(spec) = combined_specs.get(&def_idx) {
            out.push_str(&condition_definitions(def, spec)?);
        }
    }

    for def_idx in theorem_emission_order(model, &combined_specs, &rust_condition_helpers)? {
        let def = &model.defs[def_idx];
        let spec = combined_specs.get(&def_idx);
        let step_theorem = step_soundness_theorem(def, spec, &all_condition_simp_names)?;
        let body_theorem = body_obligations_theorem(def, spec, &all_condition_simp_names);
        if spec.is_some_and(ResolvedSpec::has_explicit_lean_proof) {
            out.push_str(&step_theorem);
            out.push_str(&body_theorem);
        } else {
            out.push_str(&body_theorem);
            out.push_str(&step_theorem);
        }
    }

    out.push_str(&format!("end {}\n", model.namespace));
    if !allow_sorry {
        if let Some(admission) = lean_admission_keyword(&out) {
            anyhow::bail!(
                "generated Anneal Lean contains the proof admission `{admission}`; \
                 replace it with a proof or pass `--allow-sorry` explicitly"
            );
        }
    }
    if let Some(declaration) = lean_unsafe_declaration_keyword(&out) {
        anyhow::bail!(
            "generated Anneal Lean contains the trust-extending declaration \
             `{declaration}`; raw Lean fragments may not introduce axioms"
        );
    }
    Ok(Some(GeneratedSoundness { lean: out, matched_specs }))
}

/// Orders generated step theorems so every automatically discharged obligation
/// sees the `[step]` theorems for its callees. Rust condition helpers do not get
/// standalone soundness theorems, so their call edges are projected through to
/// the function whose condition invokes them.
fn theorem_emission_order(
    model: &SoundnessModel,
    specs: &std::collections::BTreeMap<usize, ResolvedSpec>,
    rust_condition_helpers: &std::collections::BTreeSet<usize>,
) -> anyhow::Result<Vec<usize>> {
    let mut theorem_roots = Vec::new();
    let mut ordinary_roots = Vec::new();
    let mut candidates_by_id = std::collections::BTreeMap::new();
    for (def_idx, def) in model
        .defs
        .iter()
        .enumerate()
        .filter(|(_, def)| !rust_condition_helpers.contains(&def.def_id))
    {
        candidates_by_id.insert(def.def_id, def_idx);
        if specs.get(&def_idx).is_some_and(|spec| spec.rust_theorem) {
            theorem_roots.push(def_idx);
        } else {
            ordinary_roots.push(def_idx);
        }
    }

    let functions_by_id = model
        .functions
        .iter()
        .map(|function| (function.def_id, function))
        .collect::<std::collections::BTreeMap<_, _>>();
    let mut dependencies = std::collections::BTreeMap::new();
    for &def_idx in candidates_by_id.values() {
        let spec = specs.get(&def_idx);
        let def = &model.defs[def_idx];
        let mut source_ids = vec![def.def_id];
        if let Some(spec) = spec {
            source_ids.extend(spec.conditions().filter_map(ResolvedCondition::helper_def_id));
        }

        let mut pending = Vec::new();
        for source_id in source_ids {
            if let Some(source) = functions_by_id.get(&source_id) {
                pending.extend(source.direct_calls.iter().copied());
            }
        }
        let mut visited = std::collections::BTreeSet::new();
        let mut def_dependencies = std::collections::BTreeSet::new();
        while let Some(callee_id) = pending.pop() {
            if let Some(&callee_idx) = candidates_by_id.get(&callee_id) {
                def_dependencies.insert(callee_idx);
                continue;
            }
            if visited.insert(callee_id)
                && let Some(callee) = functions_by_id.get(&callee_id)
            {
                pending.extend(callee.direct_calls.iter().copied());
            }
        }
        dependencies.insert(def_idx, def_dependencies);
    }

    // An explicit Lean proof is a permitted recursion-cycle cut, but it still
    // needs every ordinary, acyclic callee theorem that its proof may invoke.
    // Drop only those outgoing edges from an explicit proof which participate
    // in a dependency cycle.
    let explicit_proofs = specs
        .iter()
        .filter_map(|(&def_idx, spec)| spec.has_explicit_lean_proof().then_some(def_idx))
        .collect::<Vec<_>>();
    for def_idx in explicit_proofs {
        let cyclic = dependencies
            .get(&def_idx)
            .into_iter()
            .flatten()
            .copied()
            .filter(|&callee_idx| dependency_reaches(&dependencies, callee_idx, def_idx))
            .collect::<Vec<_>>();
        if let Some(callees) = dependencies.get_mut(&def_idx) {
            for callee_idx in cyclic {
                callees.remove(&callee_idx);
            }
        }
    }

    let mut states = vec![0_u8; model.defs.len()];
    let mut stack = Vec::new();
    let mut order = Vec::new();
    for def_idx in theorem_roots.into_iter().chain(ordinary_roots) {
        visit_theorem_dependency(
            def_idx,
            model,
            &dependencies,
            &mut states,
            &mut stack,
            &mut order,
        )?;
    }
    Ok(order)
}

fn dependency_reaches(
    dependencies: &std::collections::BTreeMap<usize, std::collections::BTreeSet<usize>>,
    start: usize,
    target: usize,
) -> bool {
    let mut pending = vec![start];
    let mut visited = std::collections::BTreeSet::new();
    while let Some(current) = pending.pop() {
        if current == target {
            return true;
        }
        if visited.insert(current)
            && let Some(next) = dependencies.get(&current)
        {
            pending.extend(next.iter().copied());
        }
    }
    false
}

fn visit_theorem_dependency(
    def_idx: usize,
    model: &SoundnessModel,
    dependencies: &std::collections::BTreeMap<usize, std::collections::BTreeSet<usize>>,
    states: &mut [u8],
    stack: &mut Vec<usize>,
    order: &mut Vec<usize>,
) -> anyhow::Result<()> {
    match states[def_idx] {
        2 => return Ok(()),
        1 => {
            let cycle_start = stack.iter().position(|&idx| idx == def_idx).unwrap_or(0);
            let mut cycle = stack[cycle_start..]
                .iter()
                .map(|&idx| model.defs[idx].rust_name.clone())
                .collect::<Vec<_>>();
            cycle.push(model.defs[def_idx].rust_name.clone());
            anyhow::bail!(
                "automatic Anneal proofs contain a recursive dependency cycle: {}. \
                 Add an explicit Lean `proof(...)` to at least one function in the cycle",
                cycle.join(" -> ")
            );
        }
        _ => {}
    }

    states[def_idx] = 1;
    stack.push(def_idx);
    if let Some(callees) = dependencies.get(&def_idx) {
        for &callee_idx in callees {
            visit_theorem_dependency(callee_idx, model, dependencies, states, stack, order)?;
        }
    }
    stack.pop();
    states[def_idx] = 2;
    order.push(def_idx);
    Ok(())
}

fn resolve_function_spec(
    model: &SoundnessModel,
    def: &ResultDef,
    spec: &crate::annotation_scan::FunctionSpec,
) -> anyhow::Result<ResolvedSpec> {
    let fallback_owner = fallback_condition_owner(def, spec);
    let trait_self_type = trait_self_lean_type(model, def, spec);
    Ok(ResolvedSpec {
        rust_name: spec.rust_name.clone(),
        proof: spec.proof.clone(),
        rust_theorem: spec.rust_theorem,
        requires: resolve_function_conditions(
            model,
            def,
            &spec.spec.requires,
            ConditionRole::Pre,
            &fallback_owner,
            trait_self_type.as_deref(),
        )?,
        ensures: resolve_function_conditions(
            model,
            def,
            &spec.spec.ensures,
            ConditionRole::Post,
            &fallback_owner,
            trait_self_type.as_deref(),
        )?,
        invariants: resolve_function_conditions(
            model,
            def,
            &spec.spec.invariants,
            ConditionRole::Inv,
            &fallback_owner,
            trait_self_type.as_deref(),
        )?,
    })
}

fn resolve_function_conditions(
    model: &SoundnessModel,
    def: &ResultDef,
    props: &[Prop],
    role: ConditionRole,
    fallback_owner: &RustConditionOwner,
    trait_self_type: Option<&str>,
) -> anyhow::Result<Vec<ResolvedCondition>> {
    let mut rust_idx = 0;
    props
        .iter()
        .map(|prop| {
            let mut args = def.args.clone();
            if role.binds_result() {
                args.push("result".into());
            }
            let helper_idx = rust_idx;
            if matches!(prop, Prop::Rust(_)) {
                rust_idx += 1;
            }
            resolve_condition(
                model,
                prop,
                role.rust_role(),
                helper_idx,
                fallback_owner,
                Some(def),
                trait_self_type,
                args,
            )
        })
        .collect()
}

fn resolve_condition(
    model: &SoundnessModel,
    prop: &Prop,
    role: RustConditionRole,
    idx: usize,
    fallback_owner: &RustConditionOwner,
    target_def: Option<&ResultDef>,
    trait_self_type: Option<&str>,
    mut args: Vec<String>,
) -> anyhow::Result<ResolvedCondition> {
    match prop {
        Prop::Lean(lean) => Ok(ResolvedCondition::Lean(lean.clone())),
        Prop::Rust(rust) => {
            let helper_name = rust.helper_name(role, idx);
            let owner = rust.owner.as_ref().unwrap_or(fallback_owner);
            let helper = resolve_rust_condition_helper(model, owner, &helper_name, target_def)?;
            if matches!(owner, RustConditionOwner::TraitMethod { .. })
                && helper.name.ends_with(".default")
            {
                let self_type = trait_self_type.ok_or_else(|| {
                    anyhow::anyhow!(
                        "could not resolve a concrete Lean self type for trait condition helper `{}`",
                        helper.rust_name
                    )
                })?;
                args.insert(0, self_type.into());
            }
            Ok(ResolvedCondition::Rust {
                explicit_name: rust.name.clone(),
                helper_def_id: helper.def_id,
                helper_lean_name: helper.name.clone(),
                args,
            })
        }
    }
}

fn trait_self_lean_type(
    model: &SoundnessModel,
    def: &ResultDef,
    spec: &crate::annotation_scan::FunctionSpec,
) -> Option<String> {
    let crate::annotation_scan::FunctionMatchHint::TraitImpl { self_ty, .. } = &spec.match_hint
    else {
        return None;
    };
    let candidates =
        model.types.iter().filter(|ty| ty.rust_basename() == self_ty).collect::<Vec<_>>();
    let exact = candidates
        .iter()
        .copied()
        .filter(|ty| def.rust_name.contains(&ty.rust_name))
        .collect::<Vec<_>>();
    match exact.as_slice() {
        [ty] => Some(ty.name.clone()),
        [] if candidates.len() == 1 => Some(candidates[0].name.clone()),
        _ => primitive_rust_type_to_lean(self_ty).map(str::to_string),
    }
}

fn primitive_rust_type_to_lean(rust: &str) -> Option<&'static str> {
    match rust {
        "bool" => Some("Bool"),
        "char" => Some("Char"),
        "u8" => Some("Std.U8"),
        "u16" => Some("Std.U16"),
        "u32" => Some("Std.U32"),
        "u64" => Some("Std.U64"),
        "u128" => Some("Std.U128"),
        "usize" => Some("Std.Usize"),
        "i8" => Some("Std.I8"),
        "i16" => Some("Std.I16"),
        "i32" => Some("Std.I32"),
        "i64" => Some("Std.I64"),
        "i128" => Some("Std.I128"),
        "isize" => Some("Std.Isize"),
        _ => None,
    }
}

fn fallback_condition_owner(
    def: &ResultDef,
    spec: &crate::annotation_scan::FunctionSpec,
) -> RustConditionOwner {
    match &spec.match_hint {
        crate::annotation_scan::FunctionMatchHint::Basename => RustConditionOwner::Function {
            module_path: rust_module_path(&def.rust_name),
            function: spec.rust_name.clone(),
        },
        crate::annotation_scan::FunctionMatchHint::Free { module_path } => {
            RustConditionOwner::Function {
                module_path: module_path.clone(),
                function: spec.rust_name.clone(),
            }
        }
        crate::annotation_scan::FunctionMatchHint::TraitImpl {
            trait_name, method_name, ..
        } => RustConditionOwner::TraitMethod {
            module_path: Vec::new(),
            trait_name: trait_name.clone(),
            method: method_name.clone(),
        },
    }
}

fn resolve_rust_condition_helper<'a>(
    model: &'a SoundnessModel,
    owner: &RustConditionOwner,
    helper_name: &str,
    target_def: Option<&ResultDef>,
) -> anyhow::Result<&'a TranslatedFunction> {
    let mut suffixes = rust_condition_helper_suffixes(
        owner,
        helper_name,
        target_def.map(|def| def.rust_name.as_str()),
    );
    if let RustConditionOwner::Struct { module_path, struct_name } = owner {
        let helper = format!("anneal__invariant__{helper_name}");
        let mut impl_suffixes = model
            .types
            .iter()
            .filter(|ty| {
                ty.rust_basename() == struct_name
                    && rust_module_path(&ty.rust_name).as_slice() == module_path.as_slice()
            })
            .flat_map(|ty| {
                let parent = ty.rust_name.rsplit_once("::").map(|(parent, _)| parent).unwrap_or("");
                [
                    format!("{parent}::{{impl {}}}::{helper}", ty.rust_name),
                    format!("{parent}::{{impl {}}}::{helper}", ty.rust_basename()),
                ]
                .into_iter()
                .map(|name| rust_path_segments(&name))
            })
            .collect::<Vec<_>>();
        impl_suffixes.retain(|suffix| !suffixes.contains(suffix));
        suffixes.splice(1..1, impl_suffixes);
    }
    for suffix in &suffixes {
        let matches = model
            .functions
            .iter()
            .filter(|def| rust_name_has_suffix(&def.rust_name, suffix))
            .collect::<Vec<_>>();
        match matches.as_slice() {
            [] => {}
            [helper] => return Ok(helper),
            _ => {
                anyhow::bail!(
                    "Rust Anneal condition helper suffix `{}` is ambiguous across Aeneas definitions: {}",
                    suffix.join("::"),
                    matches.iter().map(|def| def.rust_name.as_str()).collect::<Vec<_>>().join(", ")
                );
            }
        }
    }

    anyhow::bail!(
        "could not match Rust Anneal condition helper `{helper_name}` to an Aeneas definition; tried Rust name suffixes: {}",
        suffixes.iter().map(|suffix| suffix.join("::")).collect::<Vec<_>>().join(", ")
    )
}

fn rust_condition_helper_suffixes(
    owner: &RustConditionOwner,
    helper_name: &str,
    target_rust_name: Option<&str>,
) -> Vec<Vec<String>> {
    let mut suffixes = Vec::new();
    match owner {
        RustConditionOwner::Function { module_path, function } => {
            let container = format!("anneal__{function}");
            suffixes.push(
                module_path
                    .iter()
                    .cloned()
                    .chain([container.clone(), helper_name.into()])
                    .collect(),
            );
            suffixes.push(vec![container, helper_name.into()]);
        }
        RustConditionOwner::TraitMethod { module_path, trait_name, method } => {
            let helper = format!("anneal__{method}__{helper_name}");
            suffixes.push(
                module_path.iter().cloned().chain([trait_name.clone(), helper.clone()]).collect(),
            );
            if let Some((target_parent, _)) =
                target_rust_name.and_then(|name| name.rsplit_once("::"))
            {
                suffixes.push(
                    rust_path_segments(target_parent).into_iter().chain([helper.clone()]).collect(),
                );
            }
            suffixes.push(vec![trait_name.clone(), helper.clone()]);
            suffixes.push(vec![helper]);
        }
        RustConditionOwner::ImplMethod { module_path, trait_name, self_ty, method } => {
            let helper = format!("anneal__{method}__{helper_name}");
            if let Some(self_ty) = self_ty {
                suffixes.push(
                    module_path.iter().cloned().chain([self_ty.clone(), helper.clone()]).collect(),
                );
                suffixes.push(vec![self_ty.clone(), helper.clone()]);
            }
            if let Some((target_parent, _)) =
                target_rust_name.and_then(|name| name.rsplit_once("::"))
            {
                suffixes.push(
                    rust_path_segments(target_parent).into_iter().chain([helper.clone()]).collect(),
                );
            }
            if let Some(trait_name) = trait_name {
                suffixes.push(vec![trait_name.clone(), helper.clone()]);
            }
            suffixes.push(vec![helper]);
        }
        RustConditionOwner::Struct { module_path, struct_name } => {
            let helper = format!("anneal__invariant__{helper_name}");
            suffixes.push(
                module_path.iter().cloned().chain([struct_name.clone(), helper.clone()]).collect(),
            );
            suffixes.push(vec![struct_name.clone(), helper.clone()]);
            suffixes.push(vec![helper]);
        }
    }
    let mut unique = Vec::new();
    for suffix in suffixes {
        if !unique.contains(&suffix) {
            unique.push(suffix);
        }
    }
    unique
}

fn rust_name_has_suffix(name: &str, suffix: &[String]) -> bool {
    let segments = rust_path_segments(name);
    segments.ends_with(suffix)
}

fn referenced_rust_condition_helpers(
    model: &SoundnessModel,
    specs: &std::collections::BTreeMap<usize, ResolvedSpec>,
    struct_invariants: &[(usize, &crate::annotation_scan::StructInvariant)],
) -> anyhow::Result<std::collections::BTreeSet<usize>> {
    let mut helpers = specs
        .values()
        .flat_map(ResolvedSpec::conditions)
        .filter_map(ResolvedCondition::helper_def_id)
        .collect::<std::collections::BTreeSet<_>>();

    for (_, invariant) in struct_invariants {
        let fallback_owner = RustConditionOwner::Struct {
            module_path: invariant.module_path.clone(),
            struct_name: invariant.rust_name.clone(),
        };
        let mut rust_idx = 0;
        for prop in &invariant.invariants {
            let Prop::Rust(rust) = prop else {
                continue;
            };
            let helper_name = rust.helper_name(RustConditionRole::Invariant, rust_idx);
            rust_idx += 1;
            let owner = rust.owner.as_ref().unwrap_or(&fallback_owner);
            let helper = resolve_rust_condition_helper(model, owner, &helper_name, None)?;
            helpers.insert(helper.def_id);
        }
    }

    Ok(helpers)
}

fn match_struct_invariants<'a>(
    model: &SoundnessModel,
    struct_invariants: &[(usize, &'a crate::annotation_scan::StructInvariant)],
) -> anyhow::Result<
    std::collections::BTreeMap<usize, Vec<&'a crate::annotation_scan::StructInvariant>>,
> {
    let mut by_type: std::collections::BTreeMap<
        usize,
        Vec<&crate::annotation_scan::StructInvariant>,
    > = std::collections::BTreeMap::new();
    for (_, invariant) in struct_invariants {
        let matches = model
            .types
            .iter()
            .filter(|ty| type_matches_struct_invariant(ty, invariant))
            .collect::<Vec<_>>();
        match matches.as_slice() {
            [] => {
                anyhow::bail!(
                    "could not match Anneal struct invariant for `{}` in {} to generated Aeneas types",
                    invariant.rust_name,
                    invariant.path.display()
                );
            }
            [ty] => {
                by_type.entry(ty.def_id).or_default().push(*invariant);
            }
            _ => {
                anyhow::bail!(
                    "Anneal struct invariant for `{}` in {} is ambiguous across generated Aeneas types: {}",
                    invariant.rust_name,
                    invariant.path.display(),
                    matches.iter().map(|ty| ty.name.as_str()).collect::<Vec<_>>().join(", ")
                );
            }
        }
    }
    Ok(by_type)
}

fn apply_struct_invariants(
    model: &SoundnessModel,
    struct_invariants_by_type: &std::collections::BTreeMap<
        usize,
        Vec<&crate::annotation_scan::StructInvariant>,
    >,
    rust_condition_helpers: &std::collections::BTreeSet<usize>,
    combined_specs: &mut std::collections::BTreeMap<usize, ResolvedSpec>,
) -> anyhow::Result<()> {
    if struct_invariants_by_type.is_empty() {
        return Ok(());
    }

    for (def_idx, def) in model.defs.iter().enumerate() {
        if rust_condition_helpers.contains(&def.def_id) {
            continue;
        }
        for unsupported in &def.unsupported_structs {
            let Some(invariants) = struct_invariants_by_type.get(&unsupported.type_id) else {
                continue;
            };
            if invariants.iter().any(|invariant| function_in_struct_invariant_scope(def, invariant))
            {
                anyhow::bail!(
                    "function `{}` uses struct invariant type in an unsupported signature position: {}",
                    def.rust_name,
                    unsupported.description
                );
            }
        }

        let mut required = Vec::new();
        let mut ensured = Vec::new();
        for required_use in &def.required_structs {
            let Some(invariants) = struct_invariants_by_type.get(&required_use.type_id) else {
                continue;
            };
            for invariant in invariants
                .iter()
                .filter(|invariant| function_in_struct_invariant_scope(def, invariant))
            {
                required.extend(resolve_struct_invariant_conditions(
                    model,
                    invariant,
                    &required_use.value,
                )?);
            }
        }
        for ensured_use in &def.ensured_structs {
            let Some(invariants) = struct_invariants_by_type.get(&ensured_use.type_id) else {
                continue;
            };
            for invariant in invariants
                .iter()
                .filter(|invariant| function_in_struct_invariant_scope(def, invariant))
            {
                ensured.extend(resolve_struct_invariant_conditions(
                    model,
                    invariant,
                    &ensured_use.value,
                )?);
            }
        }

        if required.is_empty() && ensured.is_empty() {
            continue;
        }

        let spec = combined_specs
            .entry(def_idx)
            .or_insert_with(|| ResolvedSpec::empty(def.rust_basename().to_string()));
        spec.requires.extend(required);
        spec.ensures.extend(ensured);
    }

    Ok(())
}

fn resolve_struct_invariant_conditions(
    model: &SoundnessModel,
    invariant: &crate::annotation_scan::StructInvariant,
    value: &str,
) -> anyhow::Result<Vec<ResolvedCondition>> {
    let fallback_owner = RustConditionOwner::Struct {
        module_path: invariant.module_path.clone(),
        struct_name: invariant.rust_name.clone(),
    };
    let mut rust_idx = 0;
    invariant
        .invariants
        .iter()
        .map(|prop| match prop {
            Prop::Lean(lean) => Ok(ResolvedCondition::Lean(LeanProp {
                name: lean.name.clone(),
                attrs: lean.attrs.clone(),
                prop: replace_lean_identifier(&lean.prop, "self", value),
            })),
            Prop::Rust(_) => {
                let helper_idx = rust_idx;
                rust_idx += 1;
                resolve_condition(
                    model,
                    prop,
                    RustConditionRole::Invariant,
                    helper_idx,
                    &fallback_owner,
                    None,
                    None,
                    vec![value.into()],
                )
            }
        })
        .collect()
}

fn aux_theorem(aux: &crate::annotation_scan::AuxTheorem) -> anyhow::Result<String> {
    let mut out = String::new();
    out.push_str(&format!("/-- Anneal auxiliary theorem from `{}`. -/\n", aux.path.display()));
    match &aux.theorem.definition {
        TheoremDefinition::Lean(definition) => {
            validate_lean_identifier(&aux.theorem.name, "auxiliary theorem name")?;
            if let Some(attrs) = lean_attrs(&aux.theorem.attrs)? {
                out.push_str(&attrs);
                out.push('\n');
            }
            out.push_str("theorem ");
            out.push_str(&aux.theorem.name);
            let args = lean_args(&definition.args)?;
            if !args.is_empty() {
                out.push(' ');
                out.push_str(&args);
            }
            out.push_str(" : ");
            out.push_str(&parenthesized_lean_fragment(&definition.ty.0, "auxiliary theorem type")?);
            out.push_str(&lean_proof_body(&definition.proof)?);
            out.push('\n');
        }
    }
    Ok(out)
}

fn lean_attrs(attrs: &LeanAttrs) -> anyhow::Result<Option<String>> {
    match attrs {
        LeanAttrs::Attrs(attrs) if attrs.is_empty() => Ok(None),
        LeanAttrs::Attrs(attrs) => {
            let attrs = attrs.iter().map(lean_attr).collect::<Vec<_>>().join(", ");
            validate_balanced_lean_fragment(&attrs, "Lean attribute list")?;
            Ok(Some(format!("@[{attrs}]")))
        }
        LeanAttrs::AttrsString(attrs) if attrs.trim().is_empty() => Ok(None),
        LeanAttrs::AttrsString(attrs) => {
            validate_balanced_lean_fragment(attrs, "Lean attribute list")?;
            Ok(Some(format!("@[{}]", attrs.trim())))
        }
    }
}

fn lean_attr(attr: &LeanAttr) -> &str {
    match attr {
        LeanAttr::AttrString(attr) => attr,
    }
}

fn lean_args(args: &[LeanArgs]) -> anyhow::Result<String> {
    args.iter()
        .map(lean_arg_group)
        .filter_map(|arg| match arg {
            Ok(arg) if arg.is_empty() => None,
            result => Some(result),
        })
        .collect::<anyhow::Result<Vec<_>>>()
        .map(|args| args.join(" "))
}

fn lean_arg_group(args: &LeanArgs) -> anyhow::Result<String> {
    match args {
        LeanArgs::ArgsString(args) => {
            validate_lean_binder_groups(args, "auxiliary theorem arguments")?;
            Ok(args.trim().to_string())
        }
        LeanArgs::Args(args) => args
            .iter()
            .map(lean_arg)
            .filter_map(|arg| match arg {
                Ok(arg) if arg.is_empty() => None,
                result => Some(result),
            })
            .collect::<anyhow::Result<Vec<_>>>()
            .map(|args| args.join(" ")),
    }
}

fn lean_arg(arg: &LeanArg) -> anyhow::Result<String> {
    match arg {
        LeanArg::Arg { name, ty } => {
            validate_lean_identifier(name, "auxiliary theorem argument name")?;
            Ok(format!(
                "({name} : {})",
                parenthesized_lean_fragment(&ty.0, "auxiliary theorem argument type")?
            ))
        }
        LeanArg::ArgString(arg) => {
            let arg = arg.trim();
            if arg.starts_with(['(', '{', '[']) {
                validate_lean_binder_groups(arg, "auxiliary theorem argument")?;
                Ok(arg.to_string())
            } else {
                validate_balanced_lean_fragment(arg, "auxiliary theorem argument")?;
                Ok(format!("({arg})"))
            }
        }
    }
}

fn lean_proof_body(proof: &LeanProof) -> anyhow::Result<String> {
    match proof {
        LeanProof::TacticProof(proof) => {
            validate_balanced_lean_fragment(proof, "Lean tactic proof")?;
            Ok(format!(" := (by\n{}\n)\n", indent_lean_block(proof)))
        }
        LeanProof::TermProof(proof) => {
            validate_balanced_lean_fragment(proof, "Lean term proof")?;
            Ok(format!(" := (\n{}\n)\n", indent_lean_block(proof)))
        }
    }
}

fn step_soundness_theorem(
    def: &ResultDef,
    spec: Option<&ResolvedSpec>,
    condition_def_names: &[String],
) -> anyhow::Result<String> {
    let mut out = String::new();
    match spec {
        Some(spec) => {
            out.push_str(&format!(
                "/-- Anneal step soundness theorem for Rust function `{}`. -/\n",
                spec.rust_name
            ));
        }
        None => {
            out.push_str(&format!(
                "/-- Default Anneal step soundness theorem for Rust function `{}`. -/\n",
                def.rust_name
            ));
        }
    }
    out.push_str("@[step]\n");
    out.push_str(&format!("theorem {}", soundness_theorem_name(def)));
    let required_props =
        spec.map(|spec| required_condition_hypotheses(def, spec)).unwrap_or_default();
    let ensured_props = spec.map(|spec| ensured_condition_refs(def, spec)).unwrap_or_default();
    theorem_arguments(&mut out, &def.binders, &required_props);
    out.push_str(" :\n");
    out.push_str("  ");
    out.push_str(&def.application());
    out.push_str(" ⦃ result => ");
    out.push_str(&prop_conjunction(&ensured_props));
    out.push_str(" ⦄");
    let fallback_proof = fallback_step_proof(def, condition_def_names);
    out.push_str(&step_proof_body(spec.and_then(|spec| spec.proof.as_ref()), &fallback_proof)?);
    out.push('\n');
    Ok(out)
}

fn body_obligations_theorem(
    def: &ResultDef,
    spec: Option<&ResolvedSpec>,
    condition_def_names: &[String],
) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "/-- Diagnostic Anneal body-obligations theorem for Rust function `{}`.\n\
This checks body and callee obligations under this function's precondition, but intentionally \
does not assert this function's own successful-result postcondition. -/\n",
        def.rust_name
    ));
    out.push_str(&format!("theorem {}", body_obligations_theorem_name(def)));
    let required_props =
        spec.map(|spec| required_condition_hypotheses(def, spec)).unwrap_or_default();
    theorem_arguments(&mut out, &def.binders, &required_props);
    out.push_str(" :\n");
    out.push_str("  ");
    out.push_str(&def.application());
    out.push_str(" ⦃ result => True ⦄ := by\n");
    if let Some(spec) = spec.filter(|spec| spec.has_explicit_lean_proof()) {
        let mut args = def.args.clone();
        args.extend(required_condition_hypotheses(def, spec).into_iter().map(|(name, _)| name));
        let application = if args.is_empty() {
            soundness_theorem_name(def)
        } else {
            format!("{} {}", soundness_theorem_name(def), args.join(" "))
        };
        out.push_str(&format!(
            "  apply Aeneas.Std.WP.spec_mono ({application})\n  intro _ _\n  trivial"
        ));
    } else {
        out.push_str(&indent_lean_block(&fallback_step_proof(def, condition_def_names)));
    }
    out.push_str("\n\n");
    out
}

fn fallback_step_proof(def: &ResultDef, condition_def_names: &[String]) -> String {
    let mut simp_args = vec!["Std.Do.WP.wp".to_string(), "Std.Do.PredTrans.apply".to_string()];
    simp_args.extend(condition_def_names.iter().cloned());
    let simp_args = simp_args.join(", ");
    format!(
        "unfold {}\nstep* by\n  first | assumption | simp_all [{}] | simp [{}]\nall_goals try assumption\nall_goals try simp_all [{}]\nall_goals try simp [{}]\nall_goals try step*\nall_goals try assumption\nall_goals try simp_all [{}]\nall_goals try simp [{}]",
        def.name, simp_args, simp_args, simp_args, simp_args, simp_args, simp_args
    )
}

fn type_matches_struct_invariant(
    ty: &TypeDef,
    invariant: &crate::annotation_scan::StructInvariant,
) -> bool {
    ty.rust_basename() == invariant.rust_name
        && rust_module_path(&ty.rust_name) == invariant.module_path
}

fn function_in_struct_invariant_scope(
    def: &ResultDef,
    invariant: &crate::annotation_scan::StructInvariant,
) -> bool {
    let module_path = rust_module_path(&def.rust_name);
    module_path.starts_with(&invariant.module_path)
}

fn rust_module_path(name: &str) -> Vec<String> {
    let mut segments = rust_path_segments(name);
    if !segments.is_empty() {
        segments.remove(0);
    }
    segments.pop();
    segments
}

fn rust_path_segments(name: &str) -> Vec<String> {
    name.split("::")
        .flat_map(|segment| segment.split('.'))
        .filter(|segment| !segment.is_empty())
        .map(str::to_string)
        .collect()
}

fn replace_lean_identifier(source: &str, from: &str, to: &str) -> String {
    let mut out = String::new();
    let mut rest = source;
    while let Some(idx) = rest.find(from) {
        let (prefix, suffix) = rest.split_at(idx);
        out.push_str(prefix);
        let after = &suffix[from.len()..];
        let before_ok = prefix.chars().last().is_none_or(|ch| !is_lean_ident_char(ch));
        let after_ok = after.chars().next().is_none_or(|ch| !is_lean_ident_char(ch));
        if before_ok && after_ok {
            out.push_str(to);
        } else {
            out.push_str(from);
        }
        rest = after;
    }
    out.push_str(rest);
    out
}

fn is_lean_ident_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '_' || ch == '\''
}

#[derive(Clone, Copy)]
enum ConditionRole {
    Pre,
    Inv,
    Post,
}

impl ConditionRole {
    const fn stem(self) -> &'static str {
        match self {
            Self::Pre => "pre",
            Self::Inv => "inv",
            Self::Post => "post",
        }
    }

    const fn description(self) -> &'static str {
        match self {
            Self::Pre => "precondition",
            Self::Inv => "invariant",
            Self::Post => "postcondition",
        }
    }

    const fn binds_result(self) -> bool {
        matches!(self, Self::Post)
    }

    const fn rust_role(self) -> RustConditionRole {
        match self {
            Self::Pre => RustConditionRole::Pre,
            Self::Inv => RustConditionRole::Invariant,
            Self::Post => RustConditionRole::Post,
        }
    }
}

fn condition_definitions(def: &ResultDef, spec: &ResolvedSpec) -> anyhow::Result<String> {
    let mut out = String::new();
    for (idx, condition) in spec.requires.iter().enumerate() {
        out.push_str(&condition_definition(def, ConditionRole::Pre, idx, condition)?);
    }
    for (idx, condition) in spec.invariants.iter().enumerate() {
        out.push_str(&condition_definition(def, ConditionRole::Inv, idx, condition)?);
    }
    for (idx, condition) in spec.ensures.iter().enumerate() {
        out.push_str(&condition_definition(def, ConditionRole::Post, idx, condition)?);
    }
    Ok(out)
}

fn condition_definition(
    def: &ResultDef,
    role: ConditionRole,
    idx: usize,
    condition: &ResolvedCondition,
) -> anyhow::Result<String> {
    let ResolvedCondition::Lean(lean) = condition else {
        return Ok(String::new());
    };
    validate_balanced_lean_fragment(&lean.prop, "Lean condition proposition")?;
    let mut out = String::new();
    out.push_str(&format!(
        "/-- Anneal {} for Rust function `{}`. -/\n",
        role.description(),
        def.rust_name
    ));
    out.push_str("@[simp]\n");
    out.push_str("def ");
    out.push_str(&condition_def_name(def, role, idx, condition));
    let binders = condition_binders(def, role);
    if !binders.is_empty() {
        out.push(' ');
        out.push_str(&binders.join(" "));
    }
    out.push_str(" : Prop :=\n");
    out.push_str(&indent_lean_block(&format!("({})", lean.prop.trim())));
    out.push_str("\n\n");
    Ok(out)
}

fn condition_binders(def: &ResultDef, role: ConditionRole) -> Vec<String> {
    let mut binders = def.binders.clone();
    if role.binds_result() {
        binders.push(format!("(result : {})", def.output_type));
    }
    binders
}

fn theorem_arguments(out: &mut String, binders: &[String], hypotheses: &[(String, String)]) {
    for binder in binders {
        out.push_str("\n    ");
        out.push_str(binder);
    }
    if !hypotheses.is_empty() {
        out.push_str("\n    ");
        out.push_str(ANNEAL_HYPOTHESIS_SEPARATOR);
        for (name, prop) in hypotheses {
            out.push_str("\n    (");
            out.push_str(name);
            out.push_str(" : ");
            out.push_str(prop);
            out.push(')');
        }
    }
}

fn required_condition_hypotheses(def: &ResultDef, spec: &ResolvedSpec) -> Vec<(String, String)> {
    let mut refs = spec
        .requires
        .iter()
        .enumerate()
        .map(|(idx, condition)| {
            (
                condition_hypothesis_name(ConditionRole::Pre, idx, condition),
                condition_application(def, ConditionRole::Pre, idx, condition),
            )
        })
        .collect::<Vec<_>>();
    refs.extend(spec.invariants.iter().enumerate().map(|(idx, condition)| {
        (
            condition_hypothesis_name(ConditionRole::Inv, idx, condition),
            condition_application(def, ConditionRole::Inv, idx, condition),
        )
    }));
    refs
}

fn ensured_condition_refs(def: &ResultDef, spec: &ResolvedSpec) -> Vec<String> {
    let mut refs = spec
        .ensures
        .iter()
        .enumerate()
        .map(|(idx, condition)| condition_application(def, ConditionRole::Post, idx, condition))
        .collect::<Vec<_>>();
    refs.extend(
        spec.invariants
            .iter()
            .enumerate()
            .map(|(idx, condition)| condition_application(def, ConditionRole::Inv, idx, condition)),
    );
    if spec.rust_theorem {
        refs.push("result = true".into());
    }
    refs
}

fn condition_application(
    def: &ResultDef,
    role: ConditionRole,
    idx: usize,
    condition: &ResolvedCondition,
) -> String {
    match condition {
        ResolvedCondition::Lean(_) => {
            let mut args = def.args.clone();
            if role.binds_result() {
                args.push("result".into());
            }
            let name = condition_def_name(def, role, idx, condition);
            if args.is_empty() { name } else { format!("{name} {}", args.join(" ")) }
        }
        ResolvedCondition::Rust { helper_lean_name, args, .. } => {
            let application = if args.is_empty() {
                helper_lean_name.clone()
            } else {
                format!("{helper_lean_name} {}", args.join(" "))
            };
            format!("{application} ⦃ holds => holds = true ⦄")
        }
    }
}

fn condition_simp_names(def: &ResultDef, spec: &ResolvedSpec) -> Vec<String> {
    spec.requires
        .iter()
        .enumerate()
        .map(|(idx, condition)| condition_simp_name(def, ConditionRole::Pre, idx, condition))
        .chain(
            spec.invariants.iter().enumerate().map(|(idx, condition)| {
                condition_simp_name(def, ConditionRole::Inv, idx, condition)
            }),
        )
        .chain(
            spec.ensures.iter().enumerate().map(|(idx, condition)| {
                condition_simp_name(def, ConditionRole::Post, idx, condition)
            }),
        )
        .collect()
}

fn condition_simp_name(
    def: &ResultDef,
    role: ConditionRole,
    idx: usize,
    condition: &ResolvedCondition,
) -> String {
    match condition {
        ResolvedCondition::Lean(_) => condition_def_name(def, role, idx, condition),
        ResolvedCondition::Rust { helper_lean_name, .. } => helper_lean_name.clone(),
    }
}

fn condition_def_name(
    def: &ResultDef,
    role: ConditionRole,
    idx: usize,
    condition: &ResolvedCondition,
) -> String {
    let prefix = lean_identifier_suffix(&def.name);
    let suffix = condition_suffix(role, idx, condition);
    if prefix.is_empty() { suffix } else { format!("{prefix}_{suffix}") }
}

fn condition_hypothesis_name(
    role: ConditionRole,
    idx: usize,
    condition: &ResolvedCondition,
) -> String {
    let prefix = match role {
        ConditionRole::Pre => "required",
        ConditionRole::Inv => "preserved",
        ConditionRole::Post => "ensured",
    };
    format!("{prefix}_{}'", condition_suffix(role, idx, condition))
}

fn condition_suffix(role: ConditionRole, idx: usize, condition: &ResolvedCondition) -> String {
    condition
        .explicit_name()
        .map(lean_identifier_suffix)
        .filter(|suffix| !suffix.is_empty())
        .map(|suffix| format!("{}_{}", role.stem(), suffix))
        .unwrap_or_else(|| format!("{}{}", role.stem(), idx + 1))
}

fn prop_conjunction(props: &[String]) -> String {
    match props {
        [] => "True".into(),
        [prop] => format!("({prop})"),
        props => props.iter().map(|prop| format!("({prop})")).collect::<Vec<_>>().join(" ∧ "),
    }
}

fn step_proof_body(proof: Option<&Proof>, fallback_proof: &str) -> anyhow::Result<String> {
    match proof {
        Some(Proof::Lean(LeanProof::TacticProof(proof))) => {
            validate_balanced_lean_fragment(proof, "Lean tactic proof")?;
            Ok(format!(" := (by\n{}\n)\n", indent_lean_block(proof)))
        }
        Some(Proof::Lean(LeanProof::TermProof(proof))) => {
            validate_balanced_lean_fragment(proof, "Lean term proof")?;
            Ok(format!(" := (\n{}\n)\n", indent_lean_block(proof)))
        }
        Some(Proof::Auto) | None => Ok(format!(" := by\n{}\n", indent_lean_block(fallback_proof))),
    }
}

/// Finds Lean proof-admission keywords while ignoring strings and comments.
///
/// `warningAsError` is the semantic backstop, while this scan gives an earlier
/// and more focused diagnostic for the standard spellings.
fn lean_admission_keyword(source: &str) -> Option<&'static str> {
    lean_keyword(source, &["sorry", "admit", "sorryAx"])
}

fn lean_unsafe_declaration_keyword(source: &str) -> Option<&'static str> {
    lean_keyword(source, &["axiom", "constant"])
}

fn lean_keyword(source: &str, keywords: &[&'static str]) -> Option<&'static str> {
    let bytes = source.as_bytes();
    let mut idx = 0;
    while idx < bytes.len() {
        if bytes[idx..].starts_with(b"--") {
            idx += 2;
            while idx < bytes.len() && bytes[idx] != b'\n' {
                idx += 1;
            }
            continue;
        }
        if bytes[idx..].starts_with(b"/-") {
            idx += 2;
            let mut depth = 1;
            while idx < bytes.len() && depth > 0 {
                if bytes[idx..].starts_with(b"/-") {
                    depth += 1;
                    idx += 2;
                } else if bytes[idx..].starts_with(b"-/") {
                    depth -= 1;
                    idx += 2;
                } else {
                    idx += 1;
                }
            }
            continue;
        }
        if bytes[idx] == b'"' {
            idx += 1;
            while idx < bytes.len() {
                match bytes[idx] {
                    b'\\' => idx = (idx + 2).min(bytes.len()),
                    b'"' => {
                        idx += 1;
                        break;
                    }
                    _ => idx += 1,
                }
            }
            continue;
        }
        if bytes[idx].is_ascii_alphabetic() || bytes[idx] == b'_' {
            let start = idx;
            idx += 1;
            while idx < bytes.len()
                && (bytes[idx].is_ascii_alphanumeric() || bytes[idx] == b'_' || bytes[idx] == b'\'')
            {
                idx += 1;
            }
            let identifier = &source[start..idx];
            if let Some(keyword) = keywords.iter().copied().find(|keyword| *keyword == identifier) {
                return Some(keyword);
            }
            continue;
        }
        idx += 1;
    }
    None
}

fn parenthesized_lean_fragment(source: &str, description: &str) -> anyhow::Result<String> {
    validate_balanced_lean_fragment(source, description)?;
    let source = source.trim();
    if source.is_empty() {
        anyhow::bail!("{description} cannot be empty");
    }
    Ok(format!("({source})"))
}

/// Verifies that a raw Lean fragment cannot close a delimiter supplied by the
/// generator.  Every expression/type/term fragment is then emitted inside a
/// fresh pair of parentheses, so it cannot terminate its declaration and
/// inject a new top-level command.
fn validate_balanced_lean_fragment(source: &str, description: &str) -> anyhow::Result<()> {
    let bytes = source.as_bytes();
    let mut idx = 0;
    let mut stack = Vec::new();
    while idx < bytes.len() {
        if skip_lean_quoted_or_commented(source, &mut idx, description)? {
            continue;
        }
        match bytes[idx] {
            b'(' | b'[' | b'{' => {
                stack.push(bytes[idx]);
                idx += 1;
            }
            b')' | b']' | b'}' => {
                let close = bytes[idx];
                let expected_open = match close {
                    b')' => b'(',
                    b']' => b'[',
                    b'}' => b'{',
                    _ => unreachable!(),
                };
                match stack.pop() {
                    Some(open) if open == expected_open => {}
                    _ => anyhow::bail!(
                        "{description} contains an unmatched or mismatched `{}` delimiter",
                        close as char
                    ),
                }
                idx += 1;
            }
            _ => {
                idx += source[idx..].chars().next().expect("valid UTF-8").len_utf8();
            }
        }
    }
    if let Some(open) = stack.pop() {
        anyhow::bail!("{description} contains an unmatched `{}` delimiter", open as char);
    }
    Ok(())
}

fn validate_lean_binder_groups(source: &str, description: &str) -> anyhow::Result<()> {
    validate_balanced_lean_fragment(source, description)?;
    let bytes = source.as_bytes();
    let mut idx = 0;
    let mut saw_group = false;
    while idx < bytes.len() {
        while idx < bytes.len() {
            if skip_lean_quoted_or_commented(source, &mut idx, description)? {
                continue;
            }
            let ch = source[idx..].chars().next().expect("valid UTF-8");
            if ch.is_whitespace() {
                idx += ch.len_utf8();
            } else {
                break;
            }
        }
        if idx == bytes.len() {
            break;
        }
        let open = bytes[idx];
        let close = match open {
            b'(' => b')',
            b'[' => b']',
            b'{' => b'}',
            _ => anyhow::bail!(
                "{description} must be a whitespace-separated sequence of Lean binder groups"
            ),
        };
        saw_group = true;
        idx += 1;
        let mut stack = vec![open];
        while idx < bytes.len() && !stack.is_empty() {
            if skip_lean_quoted_or_commented(source, &mut idx, description)? {
                continue;
            }
            match bytes[idx] {
                b'(' | b'[' | b'{' => {
                    stack.push(bytes[idx]);
                    idx += 1;
                }
                b')' | b']' | b'}' => {
                    let found = bytes[idx];
                    let expected = match stack.pop().expect("nonempty") {
                        b'(' => b')',
                        b'[' => b']',
                        b'{' => b'}',
                        _ => unreachable!(),
                    };
                    if found != expected {
                        anyhow::bail!(
                            "{description} contains a mismatched `{}` delimiter",
                            found as char
                        );
                    }
                    idx += 1;
                }
                _ => {
                    idx += source[idx..].chars().next().expect("valid UTF-8").len_utf8();
                }
            }
        }
        if !stack.is_empty() || bytes.get(idx.wrapping_sub(1)) != Some(&close) {
            anyhow::bail!("{description} contains an unterminated Lean binder group");
        }
    }
    if !saw_group && !source.trim().is_empty() {
        anyhow::bail!(
            "{description} must be a whitespace-separated sequence of Lean binder groups"
        );
    }
    Ok(())
}

fn skip_lean_quoted_or_commented(
    source: &str,
    idx: &mut usize,
    description: &str,
) -> anyhow::Result<bool> {
    let bytes = source.as_bytes();
    if bytes[*idx..].starts_with(b"--") {
        *idx += 2;
        while *idx < bytes.len() && bytes[*idx] != b'\n' {
            *idx += 1;
        }
        return Ok(true);
    }
    if bytes[*idx..].starts_with(b"/-") {
        *idx += 2;
        let mut depth = 1;
        while *idx < bytes.len() && depth > 0 {
            if bytes[*idx..].starts_with(b"/-") {
                depth += 1;
                *idx += 2;
            } else if bytes[*idx..].starts_with(b"-/") {
                depth -= 1;
                *idx += 2;
            } else {
                *idx += 1;
            }
        }
        if depth != 0 {
            anyhow::bail!("{description} contains an unterminated Lean block comment");
        }
        return Ok(true);
    }
    if bytes[*idx] == b'"' {
        *idx += 1;
        while *idx < bytes.len() {
            match bytes[*idx] {
                b'\\' => *idx = (*idx + 2).min(bytes.len()),
                b'"' => {
                    *idx += 1;
                    return Ok(true);
                }
                _ => *idx += 1,
            }
        }
        anyhow::bail!("{description} contains an unterminated Lean string");
    }
    if source[*idx..].starts_with('«') {
        *idx += '«'.len_utf8();
        if let Some(end) = source[*idx..].find('»') {
            *idx += end + '»'.len_utf8();
            return Ok(true);
        }
        anyhow::bail!("{description} contains an unterminated Lean quoted identifier");
    }
    if bytes[*idx] == b'\''
        && (*idx == 0
            || !matches!(
                bytes[*idx - 1],
                b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'_' | b'\''
            ))
    {
        let mut cursor = *idx + 1;
        while cursor < bytes.len() {
            match bytes[cursor] {
                b'\\' => cursor = (cursor + 2).min(bytes.len()),
                b'\'' => {
                    *idx = cursor + 1;
                    return Ok(true);
                }
                _ => cursor += 1,
            }
        }
    }
    Ok(false)
}

fn validate_lean_identifier(name: &str, description: &str) -> anyhow::Result<()> {
    let mut segments = name.split('.');
    if name.is_empty()
        || segments.any(|segment| {
            let mut chars = segment.chars();
            let Some(first) = chars.next() else {
                return true;
            };
            !(first == '_' || first.is_alphabetic())
                || chars.any(|ch| !(ch == '_' || ch == '\'' || ch.is_alphanumeric()))
        })
    {
        anyhow::bail!("{description} `{name}` is not a safe Lean identifier");
    }
    Ok(())
}

fn indent_lean_block(block: &str) -> String {
    let block = block.trim();
    if block.is_empty() {
        return "  skip".into();
    }
    block.lines().map(|line| format!("  {}", line.trim_end())).collect::<Vec<_>>().join("\n")
}

fn lean_identifier_suffix(name: &str) -> String {
    let mut out = String::new();
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' {
            out.push(ch);
        } else {
            out.push('_');
        }
    }
    while out.contains("__") {
        out = out.replace("__", "_");
    }
    out.trim_matches('_').to_string()
}

fn soundness_theorem_name(def: &ResultDef) -> String {
    let suffix = lean_identifier_suffix(&def.name);
    if suffix.is_empty() { "soundness".into() } else { format!("{suffix}_soundness") }
}

fn body_obligations_theorem_name(def: &ResultDef) -> String {
    let suffix = lean_identifier_suffix(&def.name);
    if suffix.is_empty() {
        "body_obligations_soundness".into()
    } else {
        format!("{suffix}_body_obligations_soundness")
    }
}

#[derive(Debug, Deserialize)]
struct AeneasTranslation {
    #[serde(rename = "crate")]
    crate_name: String,
    #[serde(default)]
    functions: Vec<AeneasFunctionEntry>,
    #[serde(default)]
    types: Vec<AeneasTypeEntry>,
}

#[derive(Debug, Deserialize)]
struct AeneasFunctionEntry {
    def_id: usize,
    lean_name: String,
    rust_name: String,
    is_local: bool,
    is_opaque: bool,
    can_fail: bool,
    #[serde(default)]
    r#loop: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct AeneasTypeEntry {
    def_id: usize,
    lean_name: String,
    rust_name: String,
    is_local: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct LlbcCrateInfo {
    functions: Vec<LlbcFunctionInfo>,
    types: Vec<LlbcTypeInfo>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct LlbcFunctionInfo {
    def_id: usize,
    rust_name: String,
    rust_basename: String,
    arg_names: Vec<String>,
    inputs: Vec<charon_lib::ast::Ty>,
    output: charon_lib::ast::Ty,
    direct_calls: Vec<usize>,
    unsupported_reason: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct LlbcTypeInfo {
    def_id: usize,
    rust_name: String,
    rust_basename: String,
}

fn model_from_translation_json(
    translation: &AeneasTranslation,
    llbc: &LlbcCrateInfo,
) -> anyhow::Result<SoundnessModel> {
    let mut namespace = None;
    let mut defs = Vec::new();
    let mut skipped = Vec::new();
    let types = translation
        .types
        .iter()
        .filter(|ty| ty.is_local)
        .filter_map(|ty| {
            let (type_namespace, local_name) = split_lean_name(&ty.lean_name)?;
            match &namespace {
                Some(existing) if existing != &type_namespace => None,
                None => {
                    namespace = Some(type_namespace);
                    Some(())
                }
                _ => Some(()),
            }?;
            let llbc_ty = llbc.types.iter().find(|info| info.def_id == ty.def_id);
            Some(TypeDef {
                def_id: ty.def_id,
                rust_name: llbc_ty
                    .map(|info| info.rust_name.clone())
                    .unwrap_or_else(|| ty.rust_name.clone()),
                name: local_name,
            })
        })
        .collect::<Vec<_>>();
    let type_names = types
        .iter()
        .map(|ty| (ty.def_id, ty.name.clone()))
        .collect::<std::collections::BTreeMap<_, _>>();
    let functions = translation
        .functions
        .iter()
        .filter(|function| function.is_local && !function.is_opaque)
        .filter_map(|function| {
            let (_, name) = split_lean_name(&function.lean_name)?;
            let direct_calls = llbc
                .functions
                .iter()
                .find(|info| info.def_id == function.def_id)
                .map(|info| info.direct_calls.clone())
                .unwrap_or_default();
            Some(TranslatedFunction {
                def_id: function.def_id,
                rust_name: function.rust_name.clone(),
                name,
                direct_calls,
            })
        })
        .collect::<Vec<_>>();

    for function in translation.functions.iter().filter(|function| {
        function.is_local && function.can_fail && !function.is_opaque && function.r#loop.is_none()
    }) {
        let Some((function_namespace, local_name)) = split_lean_name(&function.lean_name) else {
            skipped.push(format!(
                "`{}` has sidecar Lean name `{}` without a namespace",
                function.rust_name, function.lean_name
            ));
            continue;
        };
        match &namespace {
            Some(existing) if existing != &function_namespace => {
                anyhow::bail!(
                    "Aeneas translation.json contains multiple namespaces for one artifact: `{existing}` and `{function_namespace}`"
                );
            }
            None => namespace = Some(function_namespace),
            _ => {}
        }

        let Some(info) = llbc.functions.iter().find(|info| info.def_id == function.def_id) else {
            skipped.push(format!(
                "`{}` was named by translation.json with def_id {} but could not be matched to LLBC binders",
                function.rust_name, function.def_id
            ));
            continue;
        };
        let Some(signature) = llbc_signature_info(info, &type_names) else {
            skipped.push(format!(
                "`{}` has unsupported LLBC signature: {}",
                function.rust_name,
                info.unsupported_reason.as_deref().unwrap_or("unknown reason")
            ));
            continue;
        };
        defs.push(ResultDef {
            def_id: function.def_id,
            rust_name: function.rust_name.clone(),
            name: local_name,
            binders: signature.binders,
            args: signature.args,
            output_type: signature.output_type,
            required_structs: signature.required_structs,
            ensured_structs: signature.ensured_structs,
            unsupported_structs: signature.unsupported_structs,
        });
    }

    Ok(SoundnessModel {
        namespace: namespace
            .unwrap_or_else(|| rust_crate_to_lean_namespace(&translation.crate_name)),
        defs,
        functions,
        types,
        skipped,
    })
}

fn llbc_crate_info(llbc_path: &std::path::Path) -> anyhow::Result<LlbcCrateInfo> {
    let file = std::fs::File::open(llbc_path)
        .with_context(|| format!("Failed to open {}", llbc_path.display()))?;
    let crate_data: charon_lib::export::CrateData = serde_json::from_reader(file)
        .with_context(|| format!("Failed to parse {}", llbc_path.display()))?;

    let functions = crate_data
        .translated
        .fun_decls
        .iter()
        .filter(|fun| fun.item_meta.is_local)
        .filter_map(llbc_function_info)
        .collect();
    let types = crate_data
        .translated
        .type_decls
        .iter()
        .filter(|ty| ty.item_meta.is_local)
        .filter_map(llbc_type_info)
        .collect();
    Ok(LlbcCrateInfo { functions, types })
}

fn llbc_function_info(fun: &charon_lib::ast::FunDecl) -> Option<LlbcFunctionInfo> {
    let def_id = fun.def_id.index();
    let rust_name = rust_name_from_charon(&fun.item_meta.name)?;
    let rust_basename = rust_basename(&rust_name).to_string();
    let arg_names = llbc_argument_names(fun);
    let inputs = fun.signature.inputs.clone();
    let output = fun.signature.output.clone();
    let direct_calls = llbc_direct_calls(fun);
    let unsupported_reason = unsupported_function_generics(fun);
    Some(LlbcFunctionInfo {
        def_id,
        rust_name,
        rust_basename,
        arg_names,
        inputs,
        output,
        direct_calls,
        unsupported_reason,
    })
}

fn llbc_direct_calls(fun: &charon_lib::ast::FunDecl) -> Vec<usize> {
    let mut calls = std::collections::BTreeSet::new();
    match &fun.body {
        charon_lib::ast::Body::Structured(body) => {
            collect_llbc_block_calls(&body.body, &mut calls);
        }
        charon_lib::ast::Body::TargetDispatch(targets) => {
            calls.extend(targets.values().map(|target| target.id.index()));
        }
        _ => {}
    }
    calls.into_iter().collect()
}

fn collect_llbc_block_calls(
    block: &charon_lib::ast::llbc_ast::Block,
    calls: &mut std::collections::BTreeSet<usize>,
) {
    use charon_lib::ast::llbc_ast::StatementKind;

    for statement in &block.statements {
        match &statement.kind {
            StatementKind::Call { call, on_unwind } => {
                if let charon_lib::ast::FnOperand::Regular(function) = &call.func {
                    collect_llbc_function_pointer(function, calls);
                }
                collect_llbc_block_calls(on_unwind, calls);
            }
            StatementKind::Drop { fn_ptr, on_unwind, .. } => {
                collect_llbc_function_pointer(fn_ptr, calls);
                collect_llbc_block_calls(on_unwind, calls);
            }
            StatementKind::Assert { on_unwind, .. } => {
                collect_llbc_block_calls(on_unwind, calls);
            }
            StatementKind::InlineAsm { targets, on_unwind, .. } => {
                for target in targets {
                    collect_llbc_block_calls(target, calls);
                }
                collect_llbc_block_calls(on_unwind, calls);
            }
            StatementKind::Switch(switch) => {
                for target in switch.iter_targets() {
                    collect_llbc_block_calls(target, calls);
                }
            }
            StatementKind::Loop(body) => collect_llbc_block_calls(body, calls),
            _ => {}
        }
    }
}

fn collect_llbc_function_pointer(
    function: &charon_lib::ast::FnPtr,
    calls: &mut std::collections::BTreeSet<usize>,
) {
    if let charon_lib::ast::FnPtrKind::Fun(charon_lib::ast::FunId::Regular(function)) =
        function.kind.as_ref()
    {
        calls.insert(function.index());
    }
}

fn llbc_type_info(ty: &charon_lib::ast::TypeDecl) -> Option<LlbcTypeInfo> {
    let def_id = ty.def_id.index();
    let rust_name = rust_name_from_charon(&ty.item_meta.name)?;
    let rust_basename = rust_basename(&rust_name).to_string();
    Some(LlbcTypeInfo { def_id, rust_name, rust_basename })
}

fn unsupported_function_generics(fun: &charon_lib::ast::FunDecl) -> Option<String> {
    let generics = &fun.generics;
    if generics.types.is_empty()
        && generics.const_generics.is_empty()
        && generics.trait_clauses.is_empty()
        && generics.regions_outlive.is_empty()
        && generics.types_outlive.is_empty()
        && generics.trait_type_constraints.is_empty()
    {
        None
    } else {
        Some("generic functions with type, const, trait, or outlives parameters are not yet rendered by Anneal's LLBC model".into())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct LlbcSignatureInfo {
    binders: Vec<String>,
    args: Vec<String>,
    output_type: String,
    required_structs: Vec<StructPropUse>,
    ensured_structs: Vec<StructPropUse>,
    unsupported_structs: Vec<UnsupportedStructUse>,
}

fn llbc_signature_info(
    info: &LlbcFunctionInfo,
    type_names: &std::collections::BTreeMap<usize, String>,
) -> Option<LlbcSignatureInfo> {
    if info.unsupported_reason.is_some() {
        return None;
    }

    let mut binders = Vec::new();
    let mut args = Vec::new();
    let mut required_structs = Vec::new();
    let mut ensured_structs = Vec::new();
    let mut unsupported_structs = Vec::new();
    let mut mut_ref_structs = Vec::new();
    let mut mut_ref_output_types = Vec::new();
    for (idx, ty) in info.inputs.iter().enumerate() {
        let name = info.arg_names.get(idx).cloned().unwrap_or_else(|| format!("arg{idx}"));
        let rendered_ty = lean_binder_type_from_llbc(ty, type_names)?;
        binders.push(format!("({name} : {rendered_ty})"));
        args.push(name);
        match direct_input_struct_type_id(ty) {
            Some((type_id, InputStructKind::Owned | InputStructKind::SharedRef)) => {
                required_structs.push(StructPropUse { type_id, value: args[idx].clone() });
            }
            Some((type_id, InputStructKind::MutRef)) => {
                required_structs.push(StructPropUse { type_id, value: args[idx].clone() });
                mut_ref_structs.push(type_id);
                mut_ref_output_types.push(rendered_ty);
            }
            None => {
                unsupported_structs.extend(nested_struct_uses(
                    ty,
                    &format!("argument `{}` has nested struct type", args[idx]),
                ));
            }
        }
    }

    if let Some(output_type_id) = direct_output_struct_type_id(&info.output) {
        ensured_structs.push(StructPropUse { type_id: output_type_id, value: "result".into() });
    } else if info.output.is_unit() && mut_ref_structs.len() == 1 {
        ensured_structs.push(StructPropUse { type_id: mut_ref_structs[0], value: "result".into() });
    } else {
        if info.output.is_unit() && mut_ref_structs.len() > 1 {
            for type_id in mut_ref_structs {
                unsupported_structs.push(UnsupportedStructUse {
                    type_id,
                    description: "multiple mutable reference outputs require tuple projection support"
                        .into(),
                });
            }
        }
        unsupported_structs
            .extend(nested_struct_uses(&info.output, "return type has nested struct type"));
    }

    let output_type = if info.output.is_unit() && mut_ref_output_types.len() == 1 {
        mut_ref_output_types.remove(0)
    } else {
        lean_type_from_llbc(&info.output, type_names)?
    };

    Some(LlbcSignatureInfo {
        binders,
        args,
        output_type,
        required_structs,
        ensured_structs,
        unsupported_structs,
    })
}

fn llbc_argument_names(fun: &charon_lib::ast::FunDecl) -> Vec<String> {
    let locals = match &fun.body {
        charon_lib::ast::Body::Structured(body) => Some(&body.locals),
        charon_lib::ast::Body::Unstructured(body) => Some(&body.locals),
        _ => None,
    };
    let Some(locals) = locals else {
        return (0..fun.signature.inputs.len()).map(|idx| format!("arg{idx}")).collect();
    };
    locals
        .locals
        .iter()
        .skip(1)
        .take(fun.signature.inputs.len())
        .enumerate()
        .map(|(idx, local)| {
            local
                .name
                .as_deref()
                .filter(|name| is_plain_lean_identifier(name))
                .map(str::to_string)
                .unwrap_or_else(|| format!("arg{idx}"))
        })
        .collect()
}

fn lean_binder_type_from_llbc(
    ty: &charon_lib::ast::Ty,
    type_names: &std::collections::BTreeMap<usize, String>,
) -> Option<String> {
    match ty.kind() {
        charon_lib::ast::TyKind::Ref(_, inner, _) => lean_type_from_llbc(inner, type_names),
        _ => lean_type_from_llbc(ty, type_names),
    }
}

fn lean_type_from_llbc(
    ty: &charon_lib::ast::Ty,
    type_names: &std::collections::BTreeMap<usize, String>,
) -> Option<String> {
    if ty.is_unit() {
        return Some("Unit".into());
    }
    match ty.kind() {
        charon_lib::ast::TyKind::Literal(literal) => match literal {
            charon_lib::ast::LiteralTy::Bool => Some("Bool".into()),
            charon_lib::ast::LiteralTy::Char => Some("Char".into()),
            charon_lib::ast::LiteralTy::UInt(uint) => match uint {
                charon_lib::ast::UIntTy::Usize => Some("Std.Usize".into()),
                charon_lib::ast::UIntTy::U8 => Some("Std.U8".into()),
                charon_lib::ast::UIntTy::U16 => Some("Std.U16".into()),
                charon_lib::ast::UIntTy::U32 => Some("Std.U32".into()),
                charon_lib::ast::UIntTy::U64 => Some("Std.U64".into()),
                charon_lib::ast::UIntTy::U128 => Some("Std.U128".into()),
            },
            charon_lib::ast::LiteralTy::Int(int) => match int {
                charon_lib::ast::IntTy::Isize => Some("Std.Isize".into()),
                charon_lib::ast::IntTy::I8 => Some("Std.I8".into()),
                charon_lib::ast::IntTy::I16 => Some("Std.I16".into()),
                charon_lib::ast::IntTy::I32 => Some("Std.I32".into()),
                charon_lib::ast::IntTy::I64 => Some("Std.I64".into()),
                charon_lib::ast::IntTy::I128 => Some("Std.I128".into()),
            },
            charon_lib::ast::LiteralTy::Float(_) => None,
        },
        charon_lib::ast::TyKind::Adt(decl) => match decl.id {
            charon_lib::ast::TypeId::Adt(type_id) => type_names.get(&type_id.index()).cloned(),
            charon_lib::ast::TypeId::Tuple if decl.generics.types.is_empty() => Some("Unit".into()),
            _ => None,
        },
        charon_lib::ast::TyKind::Pattern(inner, _) => lean_type_from_llbc(inner, type_names),
        _ => None,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum InputStructKind {
    Owned,
    SharedRef,
    MutRef,
}

fn direct_input_struct_type_id(ty: &charon_lib::ast::Ty) -> Option<(usize, InputStructKind)> {
    match ty.kind() {
        charon_lib::ast::TyKind::Ref(_, inner, charon_lib::ast::RefKind::Mut) => {
            direct_adt_type_id(inner).map(|type_id| (type_id, InputStructKind::MutRef))
        }
        charon_lib::ast::TyKind::Ref(_, inner, charon_lib::ast::RefKind::Shared) => {
            direct_adt_type_id(inner).map(|type_id| (type_id, InputStructKind::SharedRef))
        }
        _ => direct_adt_type_id(ty).map(|type_id| (type_id, InputStructKind::Owned)),
    }
}

fn direct_output_struct_type_id(ty: &charon_lib::ast::Ty) -> Option<usize> {
    direct_adt_type_id(ty)
}

fn direct_adt_type_id(ty: &charon_lib::ast::Ty) -> Option<usize> {
    match ty.kind() {
        charon_lib::ast::TyKind::Adt(decl) => match decl.id {
            charon_lib::ast::TypeId::Adt(type_id) => Some(type_id.index()),
            _ => None,
        },
        charon_lib::ast::TyKind::Pattern(inner, _) => direct_adt_type_id(inner),
        _ => None,
    }
}

fn nested_struct_uses(ty: &charon_lib::ast::Ty, description: &str) -> Vec<UnsupportedStructUse> {
    let mut type_ids = std::collections::BTreeSet::new();
    collect_nested_struct_type_ids(ty, true, &mut type_ids);
    type_ids
        .into_iter()
        .map(|type_id| UnsupportedStructUse { type_id, description: description.into() })
        .collect()
}

fn collect_nested_struct_type_ids(
    ty: &charon_lib::ast::Ty,
    is_root: bool,
    out: &mut std::collections::BTreeSet<usize>,
) {
    match ty.kind() {
        charon_lib::ast::TyKind::Adt(decl) => {
            if let charon_lib::ast::TypeId::Adt(type_id) = decl.id {
                if !is_root {
                    out.insert(type_id.index());
                }
            }
            for inner in &decl.generics.types {
                collect_nested_struct_type_ids(inner, false, out);
            }
        }
        charon_lib::ast::TyKind::Ref(_, inner, _) => {
            collect_nested_struct_type_ids(inner, false, out);
        }
        charon_lib::ast::TyKind::RawPtr(inner, _)
        | charon_lib::ast::TyKind::Array(inner, _)
        | charon_lib::ast::TyKind::Slice(inner)
        | charon_lib::ast::TyKind::Pattern(inner, _) => {
            collect_nested_struct_type_ids(inner, false, out);
        }
        _ => {}
    }
}

fn rust_name_from_charon(name: &charon_lib::ast::Name) -> Option<String> {
    charon_path_idents(name).map(|idents| idents.join("::"))
}

fn charon_path_idents(name: &charon_lib::ast::Name) -> Option<Vec<String>> {
    let mut idents = Vec::new();
    for elem in &name.name {
        match elem {
            charon_lib::ast::PathElem::Ident(ident, _) => idents.push(ident.clone()),
            charon_lib::ast::PathElem::Impl(charon_lib::ast::ImplElem::Trait(id)) => {
                idents.push(format!("impl_trait_{}", id.index()));
            }
            charon_lib::ast::PathElem::Impl(charon_lib::ast::ImplElem::Ty(_)) => {
                idents.push("impl".into());
            }
            charon_lib::ast::PathElem::Instantiated(_) => {
                idents.push("inst".into());
            }
            charon_lib::ast::PathElem::Target(target) => {
                idents.push(target.to_string());
            }
        }
    }
    if idents.is_empty() { None } else { Some(idents) }
}

fn split_lean_name(name: &str) -> Option<(String, String)> {
    let (namespace, local) = name.split_once('.')?;
    Some((namespace.to_string(), local.to_string()))
}

fn rust_basename(name: &str) -> &str {
    name.rsplit("::").next().unwrap_or_else(|| name.rsplit('.').next().unwrap_or(name))
}

fn def_matches_spec(def: &ResultDef, spec: &crate::annotation_scan::FunctionSpec) -> bool {
    match &spec.match_hint {
        crate::annotation_scan::FunctionMatchHint::Basename => {
            def.rust_basename() == spec.rust_name
        }
        crate::annotation_scan::FunctionMatchHint::Free { module_path } => {
            def.rust_basename() == spec.rust_name
                && rust_module_path(&def.rust_name) == *module_path
        }
        crate::annotation_scan::FunctionMatchHint::TraitImpl {
            trait_name,
            self_ty,
            method_name,
        } => {
            def.rust_basename() == method_name
                && def.rust_name.contains("{impl ")
                && def.rust_name.contains(" for ")
                && def.rust_name.contains(&format!("::{trait_name}"))
                && def.rust_name.contains(&format!("::{self_ty}"))
        }
    }
}

fn rust_crate_to_lean_namespace(crate_name: &str) -> String {
    crate_name.replace('-', "_")
}

fn is_plain_lean_identifier(name: &str) -> bool {
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !(first.is_ascii_alphabetic() || first == '_') {
        return false;
    }
    chars.all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
}

#[cfg(test)]
mod tests {
    use super::*;

    fn result_def(name: &str, binders: &[&str], args: &[&str]) -> ResultDef {
        ResultDef {
            def_id: 0,
            rust_name: format!("demo::{}", name.replace('.', "::")),
            name: name.into(),
            binders: binders.iter().map(|binder| (*binder).into()).collect(),
            args: args.iter().map(|arg| (*arg).into()).collect(),
            output_type: "Std.U32".into(),
            required_structs: Vec::new(),
            ensured_structs: Vec::new(),
            unsupported_structs: Vec::new(),
        }
    }

    fn model(mut defs: Vec<ResultDef>) -> SoundnessModel {
        for (def_id, def) in defs.iter_mut().enumerate() {
            def.def_id = def_id;
        }
        let functions = defs
            .iter()
            .map(|def| TranslatedFunction {
                def_id: def.def_id,
                rust_name: def.rust_name.clone(),
                name: def.name.clone(),
                direct_calls: Vec::new(),
            })
            .collect();
        SoundnessModel {
            namespace: "demo".into(),
            defs,
            functions,
            types: Vec::new(),
            skipped: Vec::new(),
        }
    }

    fn rust_prop(
        name: Option<&str>,
        owner: crate::annotations::RustConditionOwner,
    ) -> crate::annotations::Prop {
        crate::annotations::Prop::Rust(crate::annotations::RustProp {
            name: name.map(str::to_string),
            expr: "true".parse().unwrap(),
            owner: Some(owner),
        })
    }

    fn helper_def(rust_name: &str, lean_name: &str, binders: &[&str], args: &[&str]) -> ResultDef {
        let mut helper = result_def(lean_name, binders, args);
        helper.rust_name = rust_name.into();
        helper.name = lean_name.into();
        helper.output_type = "Bool".into();
        helper
    }

    #[test]
    fn fallback_retries_steps_after_simplifying_condition_helpers() {
        let def = result_def("f", &["(x : Std.U32)"], &["x"]);

        assert_eq!(
            fallback_step_proof(&def, &["anneal__f.post1".into()]),
            concat!(
                "unfold f\n",
                "step* by\n",
                "  first | assumption | simp_all [Std.Do.WP.wp, Std.Do.PredTrans.apply, anneal__f.post1] | simp [Std.Do.WP.wp, Std.Do.PredTrans.apply, anneal__f.post1]\n",
                "all_goals try assumption\n",
                "all_goals try simp_all [Std.Do.WP.wp, Std.Do.PredTrans.apply, anneal__f.post1]\n",
                "all_goals try simp [Std.Do.WP.wp, Std.Do.PredTrans.apply, anneal__f.post1]\n",
                "all_goals try step*\n",
                "all_goals try assumption\n",
                "all_goals try simp_all [Std.Do.WP.wp, Std.Do.PredTrans.apply, anneal__f.post1]\n",
                "all_goals try simp [Std.Do.WP.wp, Std.Do.PredTrans.apply, anneal__f.post1]",
            )
        );
    }

    #[test]
    fn emits_default_soundness_module() {
        let model = model(vec![result_def("f", &["(x : Std.U32)"], &["x"])]);

        let generated = soundness_module("Demo", &model, &[], &[], &[], false).unwrap().unwrap();

        assert!(generated.lean.contains("import Demo.Funs"));
        assert!(generated.lean.contains("namespace demo"));
        assert!(generated.lean.contains(
            "theorem f_body_obligations_soundness\n    (x : Std.U32) :\n  f x ⦃ result => True ⦄ := by"
        ));
        assert!(!generated.lean.contains("@[step]\ntheorem f_body_obligations_soundness"));
        assert!(generated.lean.contains("@[step]\ntheorem f_soundness\n    (x : Std.U32) :"));
        assert!(generated.lean.contains("f x ⦃ result => True ⦄"));
        assert!(generated.lean.contains(" := by\n  unfold f\n  step* by\n"));
        assert!(!generated.lean.contains("anneal_default_soundness"));
        assert!(!generated.lean.contains("⊢ₛ wp⟦"));
    }

    #[test]
    fn proof_only_specs_keep_the_step_proof_out_of_body_obligations() {
        let temp = tempfile::tempdir().unwrap();
        let spec = crate::annotation_scan::FunctionSpec {
            package_manifest_path: temp.path().join("Cargo.toml"),
            path: temp.path().join("src/lib.rs"),
            rust_name: "f".into(),
            match_hint: crate::annotation_scan::FunctionMatchHint::Basename,
            spec: crate::annotations::Spec::default(),
            proof: Some(crate::annotations::Proof::Lean(
                crate::annotations::LeanProof::TacticProof("exact custom_step_proof".into()),
            )),
            rust_theorem: false,
        };
        let model = model(vec![result_def("f", &["(x : Std.U32)"], &["x"])]);

        let generated =
            soundness_module("Demo", &model, &[(0, &spec)], &[], &[], false).unwrap().unwrap();

        assert!(generated.lean.contains(
            "@[step]\ntheorem f_soundness\n    (x : Std.U32) :\n  f x ⦃ result => True ⦄ := (by\n  exact custom_step_proof\n)"
        ));
        assert!(generated.lean.contains(
            "theorem f_body_obligations_soundness\n    (x : Std.U32) :\n  f x ⦃ result => True ⦄ := by\n  apply Aeneas.Std.WP.spec_mono (f_soundness x)\n  intro _ _\n  trivial"
        ));
        assert!(
            generated.lean.find("theorem f_soundness").unwrap()
                < generated.lean.find("theorem f_body_obligations_soundness").unwrap()
        );
        assert_eq!(generated.lean.matches("custom_step_proof").count(), 1);
        assert_eq!(generated.lean.matches("unfold f").count(), 0);
    }

    #[test]
    fn rust_theorems_add_true_result_and_precede_ordinary_consumers() {
        let temp = tempfile::tempdir().unwrap();
        let spec = crate::annotation_scan::FunctionSpec {
            package_manifest_path: temp.path().join("Cargo.toml"),
            path: temp.path().join("src/lib.rs"),
            rust_name: "lemma".into(),
            match_hint: crate::annotation_scan::FunctionMatchHint::Basename,
            spec: crate::annotations::Spec {
                requires: vec![],
                ensures: vec![crate::annotations::Prop::from("result = expected")],
                invariants: vec![],
            },
            proof: Some(crate::annotations::Proof::Auto),
            rust_theorem: true,
        };
        let mut lemma = result_def("lemma", &["(expected : Bool)"], &["expected"]);
        lemma.output_type = "Bool".into();
        let model = model(vec![result_def("consumer", &["(x : Bool)"], &["x"]), lemma]);

        let generated =
            soundness_module("Demo", &model, &[(0, &spec)], &[], &[], false).unwrap().unwrap();

        assert!(generated.lean.contains(
            "lemma expected ⦃ result => (lemma_post1 expected result) ∧ (result = true) ⦄"
        ));
        assert!(
            generated.lean.find("theorem lemma_body_obligations_soundness").unwrap()
                < generated.lean.find("theorem lemma_soundness").unwrap()
        );
        assert!(
            generated.lean.find("theorem lemma_soundness").unwrap()
                < generated.lean.find("theorem consumer_soundness").unwrap()
        );
        assert!(generated.lean.contains("theorem lemma_soundness"));
        assert!(generated.lean.contains("unfold lemma"));
    }

    #[test]
    fn theorem_emission_follows_call_dependencies_instead_of_source_partition() {
        let temp = tempfile::tempdir().unwrap();
        let theorem_spec = crate::annotation_scan::FunctionSpec {
            package_manifest_path: temp.path().join("Cargo.toml"),
            path: temp.path().join("src/lib.rs"),
            rust_name: "lemma".into(),
            match_hint: crate::annotation_scan::FunctionMatchHint::Basename,
            spec: crate::annotations::Spec::default(),
            proof: Some(crate::annotations::Proof::Auto),
            rust_theorem: true,
        };
        let mut lemma = result_def("lemma", &["(x : Bool)"], &["x"]);
        lemma.output_type = "Bool".into();
        let mut model = model(vec![result_def("ordinary_callee", &["(x : Bool)"], &["x"]), lemma]);
        model.functions.iter_mut().find(|function| function.def_id == 1).unwrap().direct_calls =
            vec![0];

        let generated = soundness_module("Demo", &model, &[(0, &theorem_spec)], &[], &[], false)
            .unwrap()
            .unwrap();

        assert!(
            generated.lean.find("theorem ordinary_callee_soundness").unwrap()
                < generated.lean.find("theorem lemma_soundness").unwrap()
        );
    }

    #[test]
    fn explicit_proofs_retain_acyclic_callee_dependencies() {
        let temp = tempfile::tempdir().unwrap();
        let caller_spec = crate::annotation_scan::FunctionSpec {
            package_manifest_path: temp.path().join("Cargo.toml"),
            path: temp.path().join("src/lib.rs"),
            rust_name: "caller".into(),
            match_hint: crate::annotation_scan::FunctionMatchHint::Basename,
            spec: crate::annotations::Spec::default(),
            proof: Some(crate::annotations::Proof::Lean(
                crate::annotations::LeanProof::TacticProof("unfold caller\nstep*".into()),
            )),
            rust_theorem: true,
        };
        let mut caller = result_def("caller", &[], &[]);
        caller.output_type = "Bool".into();
        let callee = result_def("callee", &[], &[]);
        let mut model = model(vec![caller, callee]);
        model.functions[0].direct_calls = vec![1];

        let generated = soundness_module("Demo", &model, &[(0, &caller_spec)], &[], &[], false)
            .unwrap()
            .unwrap();

        assert!(
            generated.lean.find("theorem callee_soundness").unwrap()
                < generated.lean.find("theorem caller_soundness").unwrap()
        );
    }

    #[test]
    fn automatic_theorem_dependency_cycles_are_rejected_with_names() {
        let mut model = model(vec![result_def("first", &[], &[]), result_def("second", &[], &[])]);
        model.functions[0].direct_calls = vec![1];
        model.functions[1].direct_calls = vec![0];

        let error = soundness_module("Demo", &model, &[], &[], &[], false).unwrap_err();
        let message = error.to_string();
        assert!(message.contains("recursive dependency cycle"));
        assert!(message.contains("demo::first -> demo::second -> demo::first"));
        assert!(message.contains("explicit Lean `proof(...)`"));
    }

    #[test]
    fn emits_annotated_postcondition_theorem_with_proof() {
        let temp = tempfile::tempdir().unwrap();
        let spec = crate::annotation_scan::FunctionSpec {
            package_manifest_path: temp.path().join("Cargo.toml"),
            path: temp.path().join("src/lib.rs"),
            rust_name: "f".into(),
            match_hint: crate::annotation_scan::FunctionMatchHint::Basename,
            spec: crate::annotations::Spec {
                requires: vec![],
                ensures: vec![crate::annotations::Prop::from("result.val >= x.val")],
                invariants: vec![],
            },
            proof: Some(crate::annotations::Proof::Lean(
                crate::annotations::LeanProof::TacticProof("simp".into()),
            )),
            rust_theorem: false,
        };
        let model = model(vec![result_def("f", &["(x : Std.U32)"], &["x"])]);

        let generated =
            soundness_module("Demo", &model, &[(0, &spec)], &[], &[], false).unwrap().unwrap();

        assert_eq!(generated.matched_specs, vec![0]);
        assert!(generated.lean.contains(
            "def f_post1 (x : Std.U32) (result : Std.U32) : Prop :=\n  (result.val >= x.val)"
        ));
        assert!(generated.lean.contains(
            "theorem f_body_obligations_soundness\n    (x : Std.U32) :\n  f x ⦃ result => True ⦄ := by"
        ));
        assert!(generated.lean.contains(
            "@[step]\ntheorem f_soundness\n    (x : Std.U32) :\n  f x ⦃ result => (f_post1 x result) ⦄"
        ));
        assert!(generated.lean.contains(" := (by\n  simp\n)\n"));
    }

    #[test]
    fn emits_precondition_as_hypothesis_argument() {
        let temp = tempfile::tempdir().unwrap();
        let spec = crate::annotation_scan::FunctionSpec {
            package_manifest_path: temp.path().join("Cargo.toml"),
            path: temp.path().join("src/lib.rs"),
            rust_name: "f".into(),
            match_hint: crate::annotation_scan::FunctionMatchHint::Basename,
            spec: crate::annotations::Spec {
                requires: vec![crate::annotations::Prop::from("x.val > 0")],
                ensures: vec![crate::annotations::Prop::from("result.val >= x.val")],
                invariants: vec![],
            },
            proof: Some(crate::annotations::Proof::Lean(
                crate::annotations::LeanProof::TacticProof("simp".into()),
            )),
            rust_theorem: false,
        };
        let model = model(vec![result_def("f", &["(x : Std.U32)"], &["x"])]);

        let generated =
            soundness_module("Demo", &model, &[(0, &spec)], &[], &[], false).unwrap().unwrap();

        assert!(generated.lean.contains("def f_pre1 (x : Std.U32) : Prop :=\n  (x.val > 0)"));
        assert!(generated.lean.contains(
            "def f_post1 (x : Std.U32) (result : Std.U32) : Prop :=\n  (result.val >= x.val)"
        ));
        assert!(
            generated
                .lean
                .contains("theorem f_soundness\n    (x : Std.U32)\n    /- -------------------- Anneal hypotheses -------------------- -/\n    (required_pre1' : f_pre1 x) :\n  f x ⦃ result => (f_post1 x result) ⦄")
        );
    }

    #[test]
    fn condition_names_are_used_as_definition_suffixes() {
        let temp = tempfile::tempdir().unwrap();
        let spec = crate::annotation_scan::FunctionSpec {
            package_manifest_path: temp.path().join("Cargo.toml"),
            path: temp.path().join("src/lib.rs"),
            rust_name: "f".into(),
            match_hint: crate::annotation_scan::FunctionMatchHint::Basename,
            spec: crate::annotations::Spec {
                requires: vec![crate::annotations::Prop::Lean(crate::annotations::LeanProp {
                    name: Some("positive".into()),
                    attrs: crate::annotations::LeanAttrs::default(),
                    prop: "x.val > 0".into(),
                })],
                ensures: vec![crate::annotations::Prop::Lean(crate::annotations::LeanProp {
                    name: Some("preserves_value".into()),
                    attrs: crate::annotations::LeanAttrs::default(),
                    prop: "result.val = x.val".into(),
                })],
                invariants: vec![],
            },
            proof: Some(crate::annotations::Proof::Lean(
                crate::annotations::LeanProof::TacticProof("simp".into()),
            )),
            rust_theorem: false,
        };
        let model = model(vec![result_def("f", &["(x : Std.U32)"], &["x"])]);

        let generated =
            soundness_module("Demo", &model, &[(0, &spec)], &[], &[], false).unwrap().unwrap();

        assert!(generated.lean.contains("def f_pre_positive (x : Std.U32) : Prop :="));
        assert!(
            generated
                .lean
                .contains("def f_post_preserves_value (x : Std.U32) (result : Std.U32) : Prop :=")
        );
        assert!(generated.lean.contains("(required_pre_positive' : f_pre_positive x)"));
        assert!(generated.lean.contains("f x ⦃ result => (f_post_preserves_value x result) ⦄"));
    }

    #[test]
    fn rust_conditions_use_lowered_helpers_without_generating_lean_definitions() {
        let temp = tempfile::tempdir().unwrap();
        let owner = crate::annotations::RustConditionOwner::Function {
            module_path: vec!["gate".into()],
            function: "f".into(),
        };
        let spec = crate::annotation_scan::FunctionSpec {
            package_manifest_path: temp.path().join("Cargo.toml"),
            path: temp.path().join("src/gate.rs"),
            rust_name: "f".into(),
            match_hint: crate::annotation_scan::FunctionMatchHint::Basename,
            spec: crate::annotations::Spec {
                requires: vec![rust_prop(None, owner.clone())],
                ensures: vec![rust_prop(Some("preserves"), owner)],
                invariants: vec![],
            },
            proof: Some(crate::annotations::Proof::Lean(
                crate::annotations::LeanProof::TacticProof("simp".into()),
            )),
            rust_theorem: false,
        };
        let mut target = result_def("gate.f", &["(x : Std.U32)"], &["x"]);
        target.rust_name = "demo::gate::f".into();
        let model = model(vec![
            target,
            helper_def(
                "demo::gate::anneal__f::pre1",
                "gate.anneal__f.pre1",
                &["(x : Std.U32)"],
                &["x"],
            ),
            helper_def(
                "demo::gate::anneal__f::preserves",
                "gate.anneal__f.preserves",
                &["(x : Std.U32)", "(ret : Std.U32)"],
                &["x", "ret"],
            ),
        ]);

        let generated =
            soundness_module("Demo", &model, &[(0, &spec)], &[], &[], false).unwrap().unwrap();

        assert!(
            generated
                .lean
                .contains("(required_pre1' : gate.anneal__f.pre1 x ⦃ holds => holds = true ⦄)")
        );
        assert!(generated.lean.contains(
            "gate.f x ⦃ result => (gate.anneal__f.preserves x result ⦃ holds => holds = true ⦄) ⦄"
        ));
        assert!(!generated.lean.contains("def gate_f_pre1"));
        assert!(!generated.lean.contains("def gate_f_post_preserves"));
        assert!(!generated.lean.contains("theorem gate_anneal_f_pre1_soundness"));
        assert!(!generated.lean.contains("theorem gate_anneal_f_preserves_soundness"));
    }

    #[test]
    fn mixed_lean_and_rust_conditions_follow_macro_helper_numbering() {
        let temp = tempfile::tempdir().unwrap();
        let owner = crate::annotations::RustConditionOwner::Function {
            module_path: Vec::new(),
            function: "f".into(),
        };
        let spec = crate::annotation_scan::FunctionSpec {
            package_manifest_path: temp.path().join("Cargo.toml"),
            path: temp.path().join("src/lib.rs"),
            rust_name: "f".into(),
            match_hint: crate::annotation_scan::FunctionMatchHint::Basename,
            spec: crate::annotations::Spec {
                requires: vec![crate::annotations::Prop::from("True"), rust_prop(None, owner)],
                ensures: vec![],
                invariants: vec![],
            },
            proof: None,
            rust_theorem: false,
        };
        let model = model(vec![
            result_def("f", &["(x : Std.U32)"], &["x"]),
            helper_def("demo::anneal__f::pre1", "anneal__f.pre1", &["(x : Std.U32)"], &["x"]),
        ]);

        let generated =
            soundness_module("Demo", &model, &[(0, &spec)], &[], &[], false).unwrap().unwrap();

        assert!(generated.lean.contains("(required_pre1' : f_pre1 x)"));
        assert!(
            generated
                .lean
                .contains("(required_pre2' : anneal__f.pre1 x ⦃ holds => holds = true ⦄)")
        );
    }

    #[test]
    fn unreferenced_anneal_prefixed_functions_still_get_soundness_theorems() {
        let mut def = result_def("anneal__ordinary", &["(x : Std.U32)"], &["x"]);
        def.rust_name = "demo::anneal__ordinary".into();
        let model = model(vec![def]);

        let generated = soundness_module("Demo", &model, &[], &[], &[], false).unwrap().unwrap();

        assert!(
            generated
                .lean
                .contains("@[step]\ntheorem anneal_ordinary_soundness\n    (x : Std.U32) :")
        );
    }

    #[test]
    fn named_rust_precondition_and_unnamed_postcondition_use_stable_helpers() {
        let temp = tempfile::tempdir().unwrap();
        let owner = crate::annotations::RustConditionOwner::Function {
            module_path: Vec::new(),
            function: "f".into(),
        };
        let spec = crate::annotation_scan::FunctionSpec {
            package_manifest_path: temp.path().join("Cargo.toml"),
            path: temp.path().join("src/lib.rs"),
            rust_name: "f".into(),
            match_hint: crate::annotation_scan::FunctionMatchHint::Basename,
            spec: crate::annotations::Spec {
                requires: vec![rust_prop(Some("positive"), owner.clone())],
                ensures: vec![rust_prop(None, owner)],
                invariants: vec![],
            },
            proof: None,
            rust_theorem: false,
        };
        let model = model(vec![
            result_def("f", &["(x : Std.U32)"], &["x"]),
            helper_def(
                "demo::anneal__f::positive",
                "anneal__f.positive",
                &["(x : Std.U32)"],
                &["x"],
            ),
            helper_def(
                "demo::anneal__f::post1",
                "anneal__f.post1",
                &["(x : Std.U32)", "(ret : Std.U32)"],
                &["x", "ret"],
            ),
        ]);

        let generated =
            soundness_module("Demo", &model, &[(0, &spec)], &[], &[], false).unwrap().unwrap();

        assert!(
            generated.lean.contains(
                "(required_pre_positive' : anneal__f.positive x ⦃ holds => holds = true ⦄)"
            )
        );
        assert!(
            generated
                .lean
                .contains("f x ⦃ result => (anneal__f.post1 x result ⦃ holds => holds = true ⦄) ⦄")
        );
        assert!(generated.lean.contains(
            "simp_all [Std.Do.WP.wp, Std.Do.PredTrans.apply, anneal__f.positive, anneal__f.post1]"
        ));
    }

    #[test]
    fn precondition_only_specs_use_default_successful_return_postcondition() {
        let temp = tempfile::tempdir().unwrap();
        let spec = crate::annotation_scan::FunctionSpec {
            package_manifest_path: temp.path().join("Cargo.toml"),
            path: temp.path().join("src/lib.rs"),
            rust_name: "f".into(),
            match_hint: crate::annotation_scan::FunctionMatchHint::Basename,
            spec: crate::annotations::Spec {
                requires: vec![crate::annotations::Prop::from("x.val > 0")],
                ensures: vec![],
                invariants: vec![],
            },
            proof: None,
            rust_theorem: false,
        };
        let model = model(vec![result_def("f", &["(x : Std.U32)"], &["x"])]);

        let generated =
            soundness_module("Demo", &model, &[(0, &spec)], &[], &[], false).unwrap().unwrap();

        assert!(generated.lean.contains(
            "theorem f_soundness\n    (x : Std.U32)\n    /- -------------------- Anneal hypotheses -------------------- -/\n    (required_pre1' : f_pre1 x) :\n  f x ⦃ result => True ⦄"
        ));
        assert!(generated.lean.contains(
            " := by\n  unfold f\n  step* by\n    first | assumption | simp_all [Std.Do.WP.wp, Std.Do.PredTrans.apply, f_pre1] | simp [Std.Do.WP.wp, Std.Do.PredTrans.apply, f_pre1]\n"
        ));
    }

    #[test]
    fn emits_unannotated_caller_step_theorem_without_inherited_precondition() {
        let temp = tempfile::tempdir().unwrap();
        let callee_spec = crate::annotation_scan::FunctionSpec {
            package_manifest_path: temp.path().join("Cargo.toml"),
            path: temp.path().join("src/lib.rs"),
            rust_name: "needs_positive".into(),
            match_hint: crate::annotation_scan::FunctionMatchHint::Basename,
            spec: crate::annotations::Spec {
                requires: vec![crate::annotations::Prop::from("y.val > 0")],
                ensures: vec![],
                invariants: vec![],
            },
            proof: None,
            rust_theorem: false,
        };
        let model = model(vec![
            ResultDef {
                def_id: 0,
                rust_name: "demo::needs_positive".into(),
                name: "needs_positive".into(),
                binders: vec!["(y : Std.U32)".into()],
                args: vec!["y".into()],
                output_type: "Std.U32".into(),
                required_structs: Vec::new(),
                ensured_structs: Vec::new(),
                unsupported_structs: Vec::new(),
            },
            ResultDef {
                def_id: 0,
                rust_name: "demo::forwards".into(),
                name: "forwards".into(),
                binders: vec!["(x : Std.U32)".into()],
                args: vec!["x".into()],
                output_type: "Std.U32".into(),
                required_structs: Vec::new(),
                ensured_structs: Vec::new(),
                unsupported_structs: Vec::new(),
            },
        ]);

        let generated = soundness_module("Demo", &model, &[(0, &callee_spec)], &[], &[], false)
            .unwrap()
            .unwrap();

        assert!(
            generated
                .lean
                .contains("def needs_positive_pre1 (y : Std.U32) : Prop :=\n  (y.val > 0)")
        );
        assert!(generated.lean.contains(
            "theorem needs_positive_soundness\n    (y : Std.U32)\n    /- -------------------- Anneal hypotheses -------------------- -/\n    (required_pre1' : needs_positive_pre1 y) :\n  needs_positive y ⦃ result => True ⦄"
        ));
        assert!(generated.lean.contains(
            "theorem forwards_soundness\n    (x : Std.U32) :\n  forwards x ⦃ result => True ⦄ := by\n  unfold forwards\n  step*"
        ));
        assert!(generated.lean.contains(
            "first | assumption | simp_all [Std.Do.WP.wp, Std.Do.PredTrans.apply, needs_positive_pre1] | simp [Std.Do.WP.wp, Std.Do.PredTrans.apply, needs_positive_pre1]"
        ));
        assert!(generated.lean.contains(
            "theorem forwards_body_obligations_soundness\n    (x : Std.U32) :\n  forwards x ⦃ result => True ⦄ := by"
        ));
        assert!(!generated.lean.contains("anneal_callsite"));
    }

    #[test]
    fn function_invariants_are_required_and_ensured() {
        let temp = tempfile::tempdir().unwrap();
        let spec = crate::annotation_scan::FunctionSpec {
            package_manifest_path: temp.path().join("Cargo.toml"),
            path: temp.path().join("src/lib.rs"),
            rust_name: "bounded".into(),
            match_hint: crate::annotation_scan::FunctionMatchHint::Basename,
            spec: crate::annotations::Spec {
                requires: vec![],
                ensures: vec![],
                invariants: vec![crate::annotations::Prop::from("x.val <= 10")],
            },
            proof: Some(crate::annotations::Proof::Lean(
                crate::annotations::LeanProof::TacticProof("simp_all".into()),
            )),
            rust_theorem: false,
        };
        let model = model(vec![result_def("bounded", &["(x : Std.U32)"], &["x"])]);

        let generated =
            soundness_module("Demo", &model, &[(0, &spec)], &[], &[], false).unwrap().unwrap();

        assert!(
            generated.lean.contains("def bounded_inv1 (x : Std.U32) : Prop :=\n  (x.val <= 10)")
        );
        assert!(
            generated
                .lean
                .contains("theorem bounded_soundness\n    (x : Std.U32)\n    /- -------------------- Anneal hypotheses -------------------- -/\n    (preserved_inv1' : bounded_inv1 x) :")
        );
        assert!(generated.lean.contains("bounded x ⦃ result => (bounded_inv1 x) ⦄"));
    }

    #[test]
    fn function_invariants_create_unannotated_caller_step_theorems() {
        let temp = tempfile::tempdir().unwrap();
        let callee_spec = crate::annotation_scan::FunctionSpec {
            package_manifest_path: temp.path().join("Cargo.toml"),
            path: temp.path().join("src/lib.rs"),
            rust_name: "bounded".into(),
            match_hint: crate::annotation_scan::FunctionMatchHint::Basename,
            spec: crate::annotations::Spec {
                requires: vec![],
                ensures: vec![],
                invariants: vec![crate::annotations::Prop::from("y.val <= 10")],
            },
            proof: Some(crate::annotations::Proof::Lean(
                crate::annotations::LeanProof::TacticProof("simp_all".into()),
            )),
            rust_theorem: false,
        };
        let model = model(vec![
            ResultDef {
                def_id: 0,
                rust_name: "demo::bounded".into(),
                name: "bounded".into(),
                binders: vec!["(y : Std.U32)".into()],
                args: vec!["y".into()],
                output_type: "Std.U32".into(),
                required_structs: Vec::new(),
                ensured_structs: Vec::new(),
                unsupported_structs: Vec::new(),
            },
            ResultDef {
                def_id: 0,
                rust_name: "demo::forwards".into(),
                name: "forwards".into(),
                binders: vec!["(x : Std.U32)".into()],
                args: vec!["x".into()],
                output_type: "Std.U32".into(),
                required_structs: Vec::new(),
                ensured_structs: Vec::new(),
                unsupported_structs: Vec::new(),
            },
        ]);

        let generated = soundness_module("Demo", &model, &[(0, &callee_spec)], &[], &[], false)
            .unwrap()
            .unwrap();

        assert!(
            generated
                .lean
                .contains("theorem bounded_soundness\n    (y : Std.U32)\n    /- -------------------- Anneal hypotheses -------------------- -/\n    (preserved_inv1' : bounded_inv1 y) :")
        );
        assert!(generated.lean.contains(
            "theorem forwards_soundness\n    (x : Std.U32) :\n  forwards x ⦃ result => True ⦄ := by\n  unfold forwards\n  step*"
        ));
        assert!(!generated.lean.contains("anneal_callsite"));
    }

    #[test]
    fn annotated_postcondition_without_proof_uses_step_fallback() {
        let temp = tempfile::tempdir().unwrap();
        let spec = crate::annotation_scan::FunctionSpec {
            package_manifest_path: temp.path().join("Cargo.toml"),
            path: temp.path().join("src/lib.rs"),
            rust_name: "f".into(),
            match_hint: crate::annotation_scan::FunctionMatchHint::Basename,
            spec: crate::annotations::Spec {
                requires: vec![],
                ensures: vec![crate::annotations::Prop::from("True")],
                invariants: vec![],
            },
            proof: None,
            rust_theorem: false,
        };
        let model = model(vec![result_def("f", &["(x : Std.U32)"], &["x"])]);

        let generated =
            soundness_module("Demo", &model, &[(0, &spec)], &[], &[], false).unwrap().unwrap();
        assert!(generated.lean.contains(" := by\n  unfold f\n  step* by\n"));
    }

    #[test]
    fn annotated_specs_match_aeneas_module_prefixes_by_basename() {
        let temp = tempfile::tempdir().unwrap();
        let spec = crate::annotation_scan::FunctionSpec {
            package_manifest_path: temp.path().join("Cargo.toml"),
            path: temp.path().join("src/gate.rs"),
            rust_name: "allowed".into(),
            match_hint: crate::annotation_scan::FunctionMatchHint::Basename,
            spec: crate::annotations::Spec {
                requires: vec![],
                ensures: vec![crate::annotations::Prop::from("True")],
                invariants: vec![],
            },
            proof: Some(crate::annotations::Proof::Lean(
                crate::annotations::LeanProof::TacticProof("trivial".into()),
            )),
            rust_theorem: false,
        };
        let model = model(vec![ResultDef {
            def_id: 0,
            rust_name: "demo::gate::allowed".into(),
            name: "gate.allowed".into(),
            binders: vec!["(g : gate.Gate)".into(), "(count : Std.U32)".into()],
            args: vec!["g".into(), "count".into()],
            output_type: "Bool".into(),
            required_structs: Vec::new(),
            ensured_structs: Vec::new(),
            unsupported_structs: Vec::new(),
        }]);

        let generated =
            soundness_module("Demo", &model, &[(7, &spec)], &[], &[], false).unwrap().unwrap();

        assert_eq!(generated.matched_specs, vec![7]);
        assert!(generated.lean.contains("gate_allowed_soundness"));
    }

    #[test]
    fn source_scanned_free_specs_match_their_full_module_path() {
        let temp = tempfile::tempdir().unwrap();
        let spec = crate::annotation_scan::FunctionSpec {
            package_manifest_path: temp.path().join("Cargo.toml"),
            path: temp.path().join("src/gate.rs"),
            rust_name: "allowed".into(),
            match_hint: crate::annotation_scan::FunctionMatchHint::Free {
                module_path: vec!["gate".into()],
            },
            spec: crate::annotations::Spec {
                requires: vec![],
                ensures: vec![crate::annotations::Prop::from("True")],
                invariants: vec![],
            },
            proof: Some(crate::annotations::Proof::Lean(
                crate::annotations::LeanProof::TacticProof("trivial".into()),
            )),
            rust_theorem: false,
        };
        let mut gate = result_def("gate.allowed", &[], &[]);
        gate.rust_name = "demo::gate::allowed".into();
        let mut other = result_def("other.allowed", &[], &[]);
        other.rust_name = "demo::other::allowed".into();
        let model = model(vec![gate, other]);

        let generated =
            soundness_module("Demo", &model, &[(7, &spec)], &[], &[], false).unwrap().unwrap();

        assert_eq!(generated.matched_specs, vec![7]);
        assert!(generated.lean.contains("theorem gate_allowed_soundness"));
        assert!(!generated.lean.contains("def other_allowed_post1"));
    }

    #[test]
    fn annotated_trait_specs_match_trait_impl_hint() {
        let temp = tempfile::tempdir().unwrap();
        let spec = crate::annotation_scan::FunctionSpec {
            package_manifest_path: temp.path().join("Cargo.toml"),
            path: temp.path().join("src/lib.rs"),
            rust_name: "bounded".into(),
            match_hint: crate::annotation_scan::FunctionMatchHint::TraitImpl {
                trait_name: "Bounded".into(),
                self_ty: "Impl".into(),
                method_name: "bounded".into(),
            },
            spec: crate::annotations::Spec {
                requires: vec![crate::annotations::Prop::from("x.val <= 10")],
                ensures: vec![],
                invariants: vec![],
            },
            proof: None,
            rust_theorem: false,
        };
        let model = model(vec![ResultDef {
            def_id: 0,
            rust_name: "trait_demo::{impl trait_demo::Bounded for trait_demo::Impl}::bounded"
                .into(),
            name: "Impl.Insts.Trait_demoBounded.bounded".into(),
            binders: vec!["(x : Std.U32)".into()],
            args: vec!["x".into()],
            output_type: "Std.U32".into(),
            required_structs: Vec::new(),
            ensured_structs: Vec::new(),
            unsupported_structs: Vec::new(),
        }]);

        let generated =
            soundness_module("Demo", &model, &[(0, &spec)], &[], &[], false).unwrap().unwrap();

        assert_eq!(generated.matched_specs, vec![0]);
        assert!(generated.lean.contains("Impl_Insts_Trait_demoBounded_bounded_soundness"));
    }

    #[test]
    fn rust_trait_conditions_resolve_associated_helper_methods() {
        let temp = tempfile::tempdir().unwrap();
        let spec = crate::annotation_scan::FunctionSpec {
            package_manifest_path: temp.path().join("Cargo.toml"),
            path: temp.path().join("src/lib.rs"),
            rust_name: "bounded".into(),
            match_hint: crate::annotation_scan::FunctionMatchHint::TraitImpl {
                trait_name: "Bounded".into(),
                self_ty: "Impl".into(),
                method_name: "bounded".into(),
            },
            spec: crate::annotations::Spec {
                requires: vec![rust_prop(
                    None,
                    crate::annotations::RustConditionOwner::TraitMethod {
                        module_path: Vec::new(),
                        trait_name: "Bounded".into(),
                        method: "bounded".into(),
                    },
                )],
                ensures: vec![],
                invariants: vec![],
            },
            proof: None,
            rust_theorem: false,
        };
        let model = model(vec![
            ResultDef {
                def_id: 0,
                rust_name: "trait_demo::{impl trait_demo::Bounded for trait_demo::Impl}::bounded"
                    .into(),
                name: "Impl.Insts.Trait_demoBounded.bounded".into(),
                binders: vec!["(x : Std.U32)".into()],
                args: vec!["x".into()],
                output_type: "Std.U32".into(),
                required_structs: Vec::new(),
                ensured_structs: Vec::new(),
                unsupported_structs: Vec::new(),
            },
            helper_def(
                "trait_demo::Bounded::anneal__bounded__pre1",
                "Bounded.anneal__bounded__pre1",
                &["(x : Std.U32)"],
                &["x"],
            ),
        ]);

        let generated =
            soundness_module("Demo", &model, &[(0, &spec)], &[], &[], false).unwrap().unwrap();

        assert!(generated.lean.contains(
            "(required_pre1' : Bounded.anneal__bounded__pre1 x ⦃ holds => holds = true ⦄)"
        ));
    }

    #[test]
    fn rust_trait_default_helpers_resolve_from_the_translation_catalog() {
        let temp = tempfile::tempdir().unwrap();
        let spec = crate::annotation_scan::FunctionSpec {
            package_manifest_path: temp.path().join("Cargo.toml"),
            path: temp.path().join("src/lib.rs"),
            rust_name: "bounded".into(),
            match_hint: crate::annotation_scan::FunctionMatchHint::TraitImpl {
                trait_name: "Bounded".into(),
                self_ty: "Impl".into(),
                method_name: "bounded".into(),
            },
            spec: crate::annotations::Spec {
                requires: vec![rust_prop(
                    Some("bounded_arg"),
                    crate::annotations::RustConditionOwner::TraitMethod {
                        module_path: Vec::new(),
                        trait_name: "Bounded".into(),
                        method: "bounded".into(),
                    },
                )],
                ensures: vec![],
                invariants: vec![],
            },
            proof: None,
            rust_theorem: false,
        };
        let mut model = model(vec![ResultDef {
            def_id: 0,
            rust_name: "trait_demo::{impl trait_demo::Bounded for trait_demo::Impl}::bounded"
                .into(),
            name: "Impl.Insts.Trait_demoBounded.bounded".into(),
            binders: vec!["(x : Std.U32)".into()],
            args: vec!["x".into()],
            output_type: "Std.U32".into(),
            required_structs: Vec::new(),
            ensured_structs: Vec::new(),
            unsupported_structs: Vec::new(),
        }]);
        model.functions.push(TranslatedFunction {
            def_id: 99,
            rust_name: "trait_demo::Bounded::anneal__bounded__bounded_arg".into(),
            name: "Bounded.anneal__bounded__bounded_arg.default".into(),
            direct_calls: Vec::new(),
        });
        model.types.push(TypeDef {
            def_id: 0,
            rust_name: "trait_demo::Impl".into(),
            name: "Impl".into(),
        });

        let generated =
            soundness_module("Demo", &model, &[(0, &spec)], &[], &[], false).unwrap().unwrap();

        assert!(generated.lean.contains(
            "(required_pre_bounded_arg' : Bounded.anneal__bounded__bounded_arg.default Impl x ⦃ holds => holds = true ⦄)"
        ));
        assert!(
            !generated
                .lean
                .contains("theorem Bounded_anneal_bounded_bounded_arg_default_soundness")
        );
    }

    #[test]
    fn primitive_trait_self_types_map_to_aeneas_lean_types() {
        let temp = tempfile::tempdir().unwrap();
        let spec = crate::annotation_scan::FunctionSpec {
            package_manifest_path: temp.path().join("Cargo.toml"),
            path: temp.path().join("src/lib.rs"),
            rust_name: "bounded".into(),
            match_hint: crate::annotation_scan::FunctionMatchHint::TraitImpl {
                trait_name: "Bounded".into(),
                self_ty: "u32".into(),
                method_name: "bounded".into(),
            },
            spec: crate::annotations::Spec::default(),
            proof: None,
            rust_theorem: false,
        };
        let model = model(vec![ResultDef {
            def_id: 0,
            rust_name: "trait_demo::{impl trait_demo::Bounded for u32}::bounded".into(),
            name: "U32.Insts.Trait_demoBounded.bounded".into(),
            binders: vec!["(x : Std.U32)".into()],
            args: vec!["x".into()],
            output_type: "Std.U32".into(),
            required_structs: Vec::new(),
            ensured_structs: Vec::new(),
            unsupported_structs: Vec::new(),
        }]);

        assert_eq!(trait_self_lean_type(&model, &model.defs[0], &spec).as_deref(), Some("Std.U32"));
    }

    #[test]
    fn rust_trait_conditions_select_the_target_impl_helper() {
        let temp = tempfile::tempdir().unwrap();
        let spec = crate::annotation_scan::FunctionSpec {
            package_manifest_path: temp.path().join("Cargo.toml"),
            path: temp.path().join("src/lib.rs"),
            rust_name: "bounded".into(),
            match_hint: crate::annotation_scan::FunctionMatchHint::TraitImpl {
                trait_name: "Bounded".into(),
                self_ty: "Impl".into(),
                method_name: "bounded".into(),
            },
            spec: crate::annotations::Spec {
                requires: vec![rust_prop(
                    None,
                    crate::annotations::RustConditionOwner::TraitMethod {
                        module_path: Vec::new(),
                        trait_name: "Bounded".into(),
                        method: "bounded".into(),
                    },
                )],
                ensures: vec![],
                invariants: vec![],
            },
            proof: None,
            rust_theorem: false,
        };
        let model = model(vec![
            ResultDef {
                def_id: 0,
                rust_name: "trait_demo::{impl trait_demo::Bounded for trait_demo::Impl}::bounded"
                    .into(),
                name: "Impl.Insts.Trait_demoBounded.bounded".into(),
                binders: vec!["(x : Std.U32)".into()],
                args: vec!["x".into()],
                output_type: "Std.U32".into(),
                required_structs: Vec::new(),
                ensured_structs: Vec::new(),
                unsupported_structs: Vec::new(),
            },
            helper_def(
                "trait_demo::{impl trait_demo::Bounded for trait_demo::Impl}::anneal__bounded__pre1",
                "Impl.anneal__bounded__pre1",
                &["(x : Std.U32)"],
                &["x"],
            ),
            helper_def(
                "trait_demo::{impl trait_demo::Bounded for trait_demo::Other}::anneal__bounded__pre1",
                "Other.anneal__bounded__pre1",
                &["(x : Std.U32)"],
                &["x"],
            ),
        ]);

        let generated =
            soundness_module("Demo", &model, &[(0, &spec)], &[], &[], false).unwrap().unwrap();

        assert!(
            generated.lean.contains(
                "(required_pre1' : Impl.anneal__bounded__pre1 x ⦃ holds => holds = true ⦄)"
            )
        );
        assert!(!generated.lean.contains("theorem Impl_anneal_bounded_pre1_soundness"));
    }

    #[test]
    fn struct_invariants_are_required_and_ensured_for_matching_functions() {
        let temp = tempfile::tempdir().unwrap();
        let invariant = crate::annotation_scan::StructInvariant {
            package_manifest_path: temp.path().join("Cargo.toml"),
            path: temp.path().join("src/lib.rs"),
            rust_name: "Counter".into(),
            module_path: Vec::new(),
            invariants: vec![crate::annotations::Prop::from("self.value.val <= 10")],
        };
        let mut model = model(vec![ResultDef {
            def_id: 0,
            rust_name: "demo::touch".into(),
            name: "touch".into(),
            binders: vec!["(counter : Counter)".into()],
            args: vec!["counter".into()],
            output_type: "Counter".into(),
            required_structs: vec![StructPropUse { type_id: 0, value: "counter".into() }],
            ensured_structs: vec![StructPropUse { type_id: 0, value: "result".into() }],
            unsupported_structs: Vec::new(),
        }]);
        model.types =
            vec![TypeDef { def_id: 0, rust_name: "demo::Counter".into(), name: "Counter".into() }];

        let generated =
            soundness_module("Demo", &model, &[], &[(0, &invariant)], &[], true).unwrap().unwrap();

        assert!(
            generated.lean.contains(
                "def touch_pre1 (counter : Counter) : Prop :=\n  (counter.value.val <= 10)"
            )
        );
        assert!(generated.lean.contains(
            "def touch_post1 (counter : Counter) (result : Counter) : Prop :=\n  (result.value.val <= 10)"
        ));
        assert!(generated.lean.contains(
            "theorem touch_soundness\n    (counter : Counter)\n    /- -------------------- Anneal hypotheses -------------------- -/\n    (required_pre1' : touch_pre1 counter) :"
        ));
        assert!(
            generated.lean.contains("touch counter ⦃ result => (touch_post1 counter result) ⦄")
        );
    }

    #[test]
    fn rust_struct_invariants_use_inherent_helpers_for_each_value() {
        let temp = tempfile::tempdir().unwrap();
        let invariant = crate::annotation_scan::StructInvariant {
            package_manifest_path: temp.path().join("Cargo.toml"),
            path: temp.path().join("src/lib.rs"),
            rust_name: "Counter".into(),
            module_path: Vec::new(),
            invariants: vec![rust_prop(
                None,
                crate::annotations::RustConditionOwner::Struct {
                    module_path: Vec::new(),
                    struct_name: "Counter".into(),
                },
            )],
        };
        let mut helper = helper_def(
            "demo::{impl demo::Counter}::anneal__invariant__inv1",
            "Counter.anneal__invariant__inv1",
            &["(this : Counter)"],
            &["this"],
        );
        helper.required_structs = vec![StructPropUse { type_id: 0, value: "this".into() }];
        let mut model = model(vec![
            ResultDef {
                def_id: 0,
                rust_name: "demo::touch".into(),
                name: "touch".into(),
                binders: vec!["(counter : Counter)".into()],
                args: vec!["counter".into()],
                output_type: "Counter".into(),
                required_structs: vec![StructPropUse { type_id: 0, value: "counter".into() }],
                ensured_structs: vec![StructPropUse { type_id: 0, value: "result".into() }],
                unsupported_structs: Vec::new(),
            },
            helper,
            helper_def(
                "demo::{impl demo::Other}::anneal__invariant__inv1",
                "Other.anneal__invariant__inv1",
                &["(this : Other)"],
                &["this"],
            ),
        ]);
        model.types = vec![
            TypeDef { def_id: 0, rust_name: "demo::Counter".into(), name: "Counter".into() },
            TypeDef { def_id: 1, rust_name: "demo::Other".into(), name: "Other".into() },
        ];

        let generated =
            soundness_module("Demo", &model, &[], &[(0, &invariant)], &[], true).unwrap().unwrap();

        assert!(generated.lean.contains(
            "(required_pre1' : Counter.anneal__invariant__inv1 counter ⦃ holds => holds = true ⦄)"
        ));
        assert!(generated.lean.contains(
            "touch counter ⦃ result => (Counter.anneal__invariant__inv1 result ⦃ holds => holds = true ⦄) ⦄"
        ));
        assert!(!generated.lean.contains("def touch_pre1"));
        assert!(!generated.lean.contains("def touch_post1"));
        assert!(!generated.lean.contains("theorem Counter_anneal_invariant_inv1_soundness"));
    }

    #[test]
    fn struct_invariants_create_step_theorems_for_callers_with_struct_args() {
        let temp = tempfile::tempdir().unwrap();
        let invariant = crate::annotation_scan::StructInvariant {
            package_manifest_path: temp.path().join("Cargo.toml"),
            path: temp.path().join("src/lib.rs"),
            rust_name: "Counter".into(),
            module_path: Vec::new(),
            invariants: vec![crate::annotations::Prop::from("self.value.val <= 10")],
        };
        let mut model = model(vec![
            ResultDef {
                def_id: 0,
                rust_name: "demo::touch".into(),
                name: "touch".into(),
                binders: vec!["(counter : Counter)".into()],
                args: vec!["counter".into()],
                output_type: "Counter".into(),
                required_structs: vec![StructPropUse { type_id: 0, value: "counter".into() }],
                ensured_structs: vec![StructPropUse { type_id: 0, value: "result".into() }],
                unsupported_structs: Vec::new(),
            },
            ResultDef {
                def_id: 0,
                rust_name: "demo::caller".into(),
                name: "caller".into(),
                binders: vec!["(c : Counter)".into()],
                args: vec!["c".into()],
                output_type: "Unit".into(),
                required_structs: vec![StructPropUse { type_id: 0, value: "c".into() }],
                ensured_structs: Vec::new(),
                unsupported_structs: Vec::new(),
            },
        ]);
        model.types =
            vec![TypeDef { def_id: 0, rust_name: "demo::Counter".into(), name: "Counter".into() }];

        let generated =
            soundness_module("Demo", &model, &[], &[(0, &invariant)], &[], true).unwrap().unwrap();

        assert!(generated.lean.contains(
            "theorem caller_soundness\n    (c : Counter)\n    /- -------------------- Anneal hypotheses -------------------- -/\n    (required_pre1' : caller_pre1 c) :\n  caller c ⦃ result => True ⦄"
        ));
        assert!(!generated.lean.contains("anneal_callsite"));
    }

    #[test]
    fn struct_invariants_reject_unsupported_nested_signature_positions() {
        let temp = tempfile::tempdir().unwrap();
        let invariant = crate::annotation_scan::StructInvariant {
            package_manifest_path: temp.path().join("Cargo.toml"),
            path: temp.path().join("src/lib.rs"),
            rust_name: "Counter".into(),
            module_path: Vec::new(),
            invariants: vec![crate::annotations::Prop::from("self.value.val <= 10")],
        };
        let mut model = model(vec![ResultDef {
            def_id: 0,
            rust_name: "demo::wrap".into(),
            name: "wrap".into(),
            binders: vec!["(counter : Counter)".into()],
            args: vec!["counter".into()],
            output_type: "Unit".into(),
            required_structs: Vec::new(),
            ensured_structs: Vec::new(),
            unsupported_structs: vec![UnsupportedStructUse {
                type_id: 0,
                description: "return type has nested struct type".into(),
            }],
        }]);
        model.types =
            vec![TypeDef { def_id: 0, rust_name: "demo::Counter".into(), name: "Counter".into() }];

        let err = soundness_module("Demo", &model, &[], &[(0, &invariant)], &[], true)
            .unwrap_err()
            .to_string();
        assert!(err.contains("unsupported signature position"));
        assert!(err.contains("return type has nested struct type"));
    }

    #[test]
    fn sidecar_model_uses_aeneas_names_and_llbc_binders() {
        let translation: AeneasTranslation = serde_json::from_str(
            r#"{
              "crate": "demo",
              "functions": [{
                "def_id": 0,
                "lean_name": "demo.gate.allowed",
                "lean_file": "Funs.lean",
                "rust_name": "demo::gate::allowed",
                "is_local": true,
                "source": {"file": "src/gate.rs", "begin_line": 1, "end_line": 3},
                "is_opaque": false,
                "can_fail": true,
                "can_diverge": false,
                "is_rec": false,
                "reducible": false
              }]
            }"#,
        )
        .unwrap();
        let u32_ty = charon_lib::ast::TyKind::Literal(charon_lib::ast::LiteralTy::UInt(
            charon_lib::ast::UIntTy::U32,
        ))
        .into_ty();
        let llbc = LlbcCrateInfo {
            functions: vec![LlbcFunctionInfo {
                def_id: 0,
                rust_name: "demo::renamed_by_rust_metadata".into(),
                rust_basename: "allowed".into(),
                arg_names: vec!["g".into(), "count".into()],
                inputs: vec![u32_ty.clone(), u32_ty.clone()],
                output: u32_ty,
                direct_calls: Vec::new(),
                unsupported_reason: None,
            }],
            types: Vec::new(),
        };

        let model = model_from_translation_json(&translation, &llbc).unwrap();

        assert_eq!(model.namespace, "demo");
        assert_eq!(model.defs[0].rust_name, "demo::gate::allowed");
        assert_eq!(model.defs[0].name, "gate.allowed");
        assert_eq!(model.defs[0].binders, vec!["(g : Std.U32)", "(count : Std.U32)"]);
        assert_eq!(model.defs[0].application(), "gate.allowed g count");
    }

    #[test]
    fn emits_auxiliary_theorems() {
        let temp = tempfile::tempdir().unwrap();
        let aux = crate::annotation_scan::AuxTheorem {
            package_manifest_path: temp.path().join("Cargo.toml"),
            path: temp.path().join("src/lib.rs"),
            theorem: crate::annotations::Theorem {
                attrs: crate::annotations::LeanAttrs::Attrs(vec![
                    crate::annotations::LeanAttr::from("simp"),
                    crate::annotations::LeanAttr::from("local"),
                ]),
                name: "always_true".into(),
                definition: crate::annotations::TheoremDefinition::Lean(
                    crate::annotations::LeanTheoremDefinition {
                        args: vec![
                            crate::annotations::LeanArgs::ArgsString("(x : Std.U32)".into()),
                            crate::annotations::LeanArgs::Args(vec![
                                crate::annotations::LeanArg::Arg {
                                    name: "h".into(),
                                    ty: crate::annotations::LeanType("x.val = x.val".into()),
                                },
                            ]),
                        ],
                        ty: crate::annotations::LeanType("True".into()),
                        proof: crate::annotations::LeanProof::TacticProof("trivial".into()),
                    },
                ),
            },
        };
        let model = model(vec![result_def("f", &["(x : Std.U32)"], &["x"])]);

        let generated =
            soundness_module("Demo", &model, &[], &[], &[&aux], false).unwrap().unwrap();

        assert!(generated.lean.contains("@[simp, local]\ntheorem always_true"));
        assert!(generated.lean.contains("(x : Std.U32) (h : (x.val = x.val)) : (True)"));
        assert!(generated.lean.contains(" := (by\n  trivial\n)\n"));
    }

    #[test]
    fn raw_lean_fragments_cannot_inject_top_level_axioms() {
        let temp = tempfile::tempdir().unwrap();
        let aux = crate::annotation_scan::AuxTheorem {
            package_manifest_path: temp.path().join("Cargo.toml"),
            path: temp.path().join("src/lib.rs"),
            theorem: crate::annotations::Theorem {
                attrs: crate::annotations::LeanAttrs::empty(),
                name: "tail".into(),
                definition: crate::annotations::TheoremDefinition::Lean(
                    crate::annotations::LeanTheoremDefinition {
                        args: Vec::new(),
                        ty: crate::annotations::LeanType(
                            "True := by trivial\naxiom forged : False\ntheorem tail : True".into(),
                        ),
                        proof: crate::annotations::LeanProof::TacticProof("trivial".into()),
                    },
                ),
            },
        };
        let model = model(vec![result_def("f", &[], &[])]);

        for allow_sorry in [false, true] {
            let error =
                soundness_module("Demo", &model, &[], &[], &[&aux], allow_sorry).unwrap_err();
            assert!(error.to_string().contains("may not introduce axioms"));
        }
    }

    #[test]
    fn proof_admissions_require_the_explicit_allow_sorry_flag() {
        let temp = tempfile::tempdir().unwrap();
        let spec = crate::annotation_scan::FunctionSpec {
            package_manifest_path: temp.path().join("Cargo.toml"),
            path: temp.path().join("src/lib.rs"),
            rust_name: "f".into(),
            match_hint: crate::annotation_scan::FunctionMatchHint::Basename,
            spec: crate::annotations::Spec::default(),
            proof: Some(crate::annotations::Proof::Lean(
                crate::annotations::LeanProof::TacticProof("sorry".into()),
            )),
            rust_theorem: false,
        };
        let model = model(vec![result_def("f", &[], &[])]);

        let error = soundness_module("Demo", &model, &[(0, &spec)], &[], &[], false).unwrap_err();
        assert!(error.to_string().contains("pass `--allow-sorry` explicitly"));

        let generated =
            soundness_module("Demo", &model, &[(0, &spec)], &[], &[], true).unwrap().unwrap();
        assert!(generated.lean.contains("  sorry"));
        assert!(!generated.lean.contains("set_option warningAsError true"));

        let mut sorry_ax_spec = spec.clone();
        sorry_ax_spec.proof = Some(crate::annotations::Proof::Lean(
            crate::annotations::LeanProof::TacticProof("exact sorryAx _ true".into()),
        ));
        let error =
            soundness_module("Demo", &model, &[(0, &sorry_ax_spec)], &[], &[], false).unwrap_err();
        assert!(error.to_string().contains("`sorryAx`"));

        let generated =
            soundness_module("Demo", &model, &[(0, &spec)], &[], &[], false).unwrap_err();
        assert!(generated.to_string().contains("pass `--allow-sorry`"));

        let clean = soundness_module("Demo", &model, &[], &[], &[], false).unwrap().unwrap();
        assert!(clean.lean.contains("set_option warningAsError true"));
    }

    #[test]
    fn admission_scanner_ignores_comments_strings_and_longer_identifiers() {
        assert_eq!(lean_admission_keyword("by\n  sorry"), Some("sorry"));
        assert_eq!(lean_admission_keyword("by\n  admit"), Some("admit"));
        assert_eq!(lean_admission_keyword("exact sorryAx _ true"), Some("sorryAx"));
        assert_eq!(
            lean_admission_keyword(
                "-- sorry\n/- outer /- admit -/ sorry -/\n\
                 def sorryMessage := \"admit and sorry\"\n\
                 exact sorryMessage"
            ),
            None
        );
    }
}
