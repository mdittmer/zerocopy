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

use crate::annotations::{LeanProof, LeanProp, Proof, Prop, RustConditionOwner, RustConditionRole};

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
        let Some(generated) =
            soundness_module(&artifact.artifact_slug(), &model, &artifact_specs, allow_sorry)?
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
struct ResultDef {
    def_id: usize,
    rust_name: String,
    name: String,
    binders: Vec<String>,
    args: Vec<String>,
    output_type: String,
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

#[derive(Clone, Debug)]
struct ResolvedSpec {
    rust_name: String,
    proof: Option<Proof>,
    requires: Vec<ResolvedCondition>,
    ensures: Vec<ResolvedCondition>,
    invariants: Vec<ResolvedCondition>,
}

impl ResolvedSpec {
    fn is_empty(&self) -> bool {
        self.proof.is_none()
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
    allow_sorry: bool,
) -> anyhow::Result<Option<GeneratedSoundness>> {
    if model.defs.is_empty() && specs.is_empty() {
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
        // the "declaration uses sorry" warning. Making warnings fatal lets the
        // generated fallback end in `all_goals sorry`: the tail is harmless
        // when the active tactics close every goal and fatal only when used.
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

    let rust_condition_helpers = referenced_rust_condition_helpers(&combined_specs);
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
    let mut roots = Vec::new();
    let mut candidates_by_id = std::collections::BTreeMap::new();
    for (def_idx, def) in model
        .defs
        .iter()
        .enumerate()
        .filter(|(_, def)| !rust_condition_helpers.contains(&def.def_id))
    {
        candidates_by_id.insert(def.def_id, def_idx);
        roots.push(def_idx);
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
    for def_idx in roots {
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
    let fallback_owner = fallback_condition_owner(spec);
    Ok(ResolvedSpec {
        rust_name: spec.rust_name.clone(),
        proof: spec.proof.clone(),
        requires: resolve_function_conditions(
            model,
            def,
            &spec.spec.requires,
            ConditionRole::Pre,
            &fallback_owner,
        )?,
        ensures: resolve_function_conditions(
            model,
            def,
            &spec.spec.ensures,
            ConditionRole::Post,
            &fallback_owner,
        )?,
        invariants: resolve_function_conditions(
            model,
            def,
            &spec.spec.invariants,
            ConditionRole::Inv,
            &fallback_owner,
        )?,
    })
}

fn resolve_function_conditions(
    model: &SoundnessModel,
    def: &ResultDef,
    props: &[Prop],
    role: ConditionRole,
    fallback_owner: &RustConditionOwner,
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
            resolve_condition(model, prop, role.rust_role(), helper_idx, fallback_owner, args)
        })
        .collect()
}

fn resolve_condition(
    model: &SoundnessModel,
    prop: &Prop,
    role: RustConditionRole,
    idx: usize,
    fallback_owner: &RustConditionOwner,
    args: Vec<String>,
) -> anyhow::Result<ResolvedCondition> {
    match prop {
        Prop::Lean(lean) => Ok(ResolvedCondition::Lean(lean.clone())),
        Prop::Rust(rust) => {
            let helper_name = rust.helper_name(role, idx);
            let owner = rust.owner.as_ref().unwrap_or(fallback_owner);
            let helper = resolve_rust_condition_helper(model, owner, &helper_name)?;
            Ok(ResolvedCondition::Rust {
                explicit_name: rust.name.clone(),
                helper_def_id: helper.def_id,
                helper_lean_name: helper.name.clone(),
                args,
            })
        }
    }
}

fn fallback_condition_owner(spec: &crate::annotation_scan::FunctionSpec) -> RustConditionOwner {
    match &spec.match_hint {
        crate::annotation_scan::FunctionMatchHint::Free { module_path } => {
            RustConditionOwner::Function {
                module_path: module_path.clone(),
                function: spec.rust_name.clone(),
            }
        }
    }
}

fn resolve_rust_condition_helper<'a>(
    model: &'a SoundnessModel,
    owner: &RustConditionOwner,
    helper_name: &str,
) -> anyhow::Result<&'a TranslatedFunction> {
    let suffixes = rust_condition_helper_suffixes(owner, helper_name);
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
    specs: &std::collections::BTreeMap<usize, ResolvedSpec>,
) -> std::collections::BTreeSet<usize> {
    specs
        .values()
        .flat_map(ResolvedSpec::conditions)
        .filter_map(ResolvedCondition::helper_def_id)
        .collect()
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
        "unfold {}\nstep* by\n  first | assumption | simp_all [{}] | simp [{}]\nall_goals try assumption\nall_goals try simp_all [{}]\nall_goals try simp [{}]\nall_goals try step*\nall_goals try assumption\nall_goals try simp_all [{}]\nall_goals try simp [{}]\nall_goals sorry",
        def.name, simp_args, simp_args, simp_args, simp_args, simp_args, simp_args
    )
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
        None => Ok(format!(" := by\n{}\n", indent_lean_block(fallback_proof))),
    }
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
    is_local: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct LlbcCrateInfo {
    functions: Vec<LlbcFunctionInfo>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct LlbcFunctionInfo {
    def_id: usize,
    arg_names: Vec<String>,
    inputs: Vec<charon_lib::ast::Ty>,
    output: charon_lib::ast::Ty,
    direct_calls: Vec<usize>,
    unsupported_reason: Option<String>,
}

fn model_from_translation_json(
    translation: &AeneasTranslation,
    llbc: &LlbcCrateInfo,
) -> anyhow::Result<SoundnessModel> {
    let mut namespace = None;
    let mut defs = Vec::new();
    let mut skipped = Vec::new();
    let type_names = translation
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
            Some((ty.def_id, local_name))
        })
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
        });
    }

    Ok(SoundnessModel {
        namespace: namespace
            .unwrap_or_else(|| rust_crate_to_lean_namespace(&translation.crate_name)),
        defs,
        functions,
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
        .map(llbc_function_info)
        .collect();
    Ok(LlbcCrateInfo { functions })
}

fn llbc_function_info(fun: &charon_lib::ast::FunDecl) -> LlbcFunctionInfo {
    let def_id = fun.def_id.index();
    let arg_names = llbc_argument_names(fun);
    let inputs = fun.signature.inputs.clone();
    let output = fun.signature.output.clone();
    let direct_calls = llbc_direct_calls(fun);
    let unsupported_reason = unsupported_function_generics(fun);
    LlbcFunctionInfo { def_id, arg_names, inputs, output, direct_calls, unsupported_reason }
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
    let mut mut_ref_output_types = Vec::new();
    for (idx, ty) in info.inputs.iter().enumerate() {
        let name = info.arg_names.get(idx).cloned().unwrap_or_else(|| format!("arg{idx}"));
        let rendered_ty = lean_binder_type_from_llbc(ty, type_names)?;
        binders.push(format!("({name} : {rendered_ty})"));
        args.push(name);
        if matches!(ty.kind(), charon_lib::ast::TyKind::Ref(_, _, charon_lib::ast::RefKind::Mut)) {
            mut_ref_output_types.push(rendered_ty);
        }
    }

    let output_type = if info.output.is_unit() && mut_ref_output_types.len() == 1 {
        mut_ref_output_types.remove(0)
    } else {
        lean_type_from_llbc(&info.output, type_names)?
    };

    Some(LlbcSignatureInfo { binders, args, output_type })
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

fn split_lean_name(name: &str) -> Option<(String, String)> {
    let (namespace, local) = name.split_once('.')?;
    Some((namespace.to_string(), local.to_string()))
}

fn rust_basename(name: &str) -> &str {
    name.rsplit("::").next().unwrap_or_else(|| name.rsplit('.').next().unwrap_or(name))
}

fn def_matches_spec(def: &ResultDef, spec: &crate::annotation_scan::FunctionSpec) -> bool {
    match &spec.match_hint {
        crate::annotation_scan::FunctionMatchHint::Free { module_path } => {
            def.rust_basename() == spec.rust_name
                && rust_module_path(&def.rust_name) == *module_path
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
        SoundnessModel { namespace: "demo".into(), defs, functions, skipped: Vec::new() }
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
                "all_goals try simp [Std.Do.WP.wp, Std.Do.PredTrans.apply, anneal__f.post1]\n",
                "all_goals sorry",
            )
        );
    }

    #[test]
    fn emits_default_soundness_module() {
        let model = model(vec![result_def("f", &["(x : Std.U32)"], &["x"])]);

        let generated = soundness_module("Demo", &model, &[], false).unwrap().unwrap();

        assert!(generated.lean.contains("import Demo.Funs"));
        assert!(generated.lean.contains("namespace demo"));
        assert!(generated.lean.contains(
            "theorem f_body_obligations_soundness\n    (x : Std.U32) :\n  f x ⦃ result => True ⦄ := by"
        ));
        assert!(!generated.lean.contains("@[step]\ntheorem f_body_obligations_soundness"));
        assert!(generated.lean.contains("@[step]\ntheorem f_soundness\n    (x : Std.U32) :"));
        assert!(generated.lean.contains("f x ⦃ result => True ⦄"));
        assert!(generated.lean.contains(" := by\n  unfold f\n  step* by\n"));
        assert_eq!(generated.lean.matches("all_goals sorry").count(), 2);
        assert!(generated.lean.contains("set_option warningAsError true"));
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
            match_hint: crate::annotation_scan::FunctionMatchHint::Free { module_path: Vec::new() },
            spec: crate::annotations::Spec::default(),
            proof: Some(crate::annotations::Proof::Lean(
                crate::annotations::LeanProof::TacticProof("exact custom_step_proof".into()),
            )),
        };
        let model = model(vec![result_def("f", &["(x : Std.U32)"], &["x"])]);

        let generated = soundness_module("Demo", &model, &[(0, &spec)], false).unwrap().unwrap();

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
    fn theorem_emission_follows_call_dependencies() {
        let mut lemma = result_def("lemma", &["(x : Bool)"], &["x"]);
        lemma.output_type = "Bool".into();
        let mut model = model(vec![result_def("ordinary_callee", &["(x : Bool)"], &["x"]), lemma]);
        model.functions.iter_mut().find(|function| function.def_id == 1).unwrap().direct_calls =
            vec![0];

        let generated = soundness_module("Demo", &model, &[], false).unwrap().unwrap();

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
            match_hint: crate::annotation_scan::FunctionMatchHint::Free { module_path: Vec::new() },
            spec: crate::annotations::Spec::default(),
            proof: Some(crate::annotations::Proof::Lean(
                crate::annotations::LeanProof::TacticProof("unfold caller\nstep*".into()),
            )),
        };
        let mut caller = result_def("caller", &[], &[]);
        caller.output_type = "Bool".into();
        let callee = result_def("callee", &[], &[]);
        let mut model = model(vec![caller, callee]);
        model.functions[0].direct_calls = vec![1];

        let generated =
            soundness_module("Demo", &model, &[(0, &caller_spec)], false).unwrap().unwrap();

        assert!(
            generated.lean.find("theorem callee_soundness").unwrap()
                < generated.lean.find("theorem caller_soundness").unwrap()
        );
    }

    #[test]
    fn automatic_dependency_cycles_are_rejected_with_names() {
        let mut model = model(vec![result_def("first", &[], &[]), result_def("second", &[], &[])]);
        model.functions[0].direct_calls = vec![1];
        model.functions[1].direct_calls = vec![0];

        let error = soundness_module("Demo", &model, &[], false).unwrap_err();
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
            match_hint: crate::annotation_scan::FunctionMatchHint::Free { module_path: Vec::new() },
            spec: crate::annotations::Spec {
                requires: vec![],
                ensures: vec![crate::annotations::Prop::from("result.val >= x.val")],
                invariants: vec![],
            },
            proof: Some(crate::annotations::Proof::Lean(
                crate::annotations::LeanProof::TacticProof("simp".into()),
            )),
        };
        let model = model(vec![result_def("f", &["(x : Std.U32)"], &["x"])]);

        let generated = soundness_module("Demo", &model, &[(0, &spec)], false).unwrap().unwrap();

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
            match_hint: crate::annotation_scan::FunctionMatchHint::Free { module_path: Vec::new() },
            spec: crate::annotations::Spec {
                requires: vec![crate::annotations::Prop::from("x.val > 0")],
                ensures: vec![crate::annotations::Prop::from("result.val >= x.val")],
                invariants: vec![],
            },
            proof: Some(crate::annotations::Proof::Lean(
                crate::annotations::LeanProof::TacticProof("simp".into()),
            )),
        };
        let model = model(vec![result_def("f", &["(x : Std.U32)"], &["x"])]);

        let generated = soundness_module("Demo", &model, &[(0, &spec)], false).unwrap().unwrap();

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
            match_hint: crate::annotation_scan::FunctionMatchHint::Free { module_path: Vec::new() },
            spec: crate::annotations::Spec {
                requires: vec![crate::annotations::Prop::Lean(crate::annotations::LeanProp {
                    name: Some("positive".into()),
                    prop: "x.val > 0".into(),
                })],
                ensures: vec![crate::annotations::Prop::Lean(crate::annotations::LeanProp {
                    name: Some("preserves_value".into()),
                    prop: "result.val = x.val".into(),
                })],
                invariants: vec![],
            },
            proof: Some(crate::annotations::Proof::Lean(
                crate::annotations::LeanProof::TacticProof("simp".into()),
            )),
        };
        let model = model(vec![result_def("f", &["(x : Std.U32)"], &["x"])]);

        let generated = soundness_module("Demo", &model, &[(0, &spec)], false).unwrap().unwrap();

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
            match_hint: crate::annotation_scan::FunctionMatchHint::Free {
                module_path: vec!["gate".into()],
            },
            spec: crate::annotations::Spec {
                requires: vec![rust_prop(None, owner.clone())],
                ensures: vec![rust_prop(Some("preserves"), owner)],
                invariants: vec![],
            },
            proof: Some(crate::annotations::Proof::Lean(
                crate::annotations::LeanProof::TacticProof("simp".into()),
            )),
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

        let generated = soundness_module("Demo", &model, &[(0, &spec)], false).unwrap().unwrap();

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
            match_hint: crate::annotation_scan::FunctionMatchHint::Free { module_path: Vec::new() },
            spec: crate::annotations::Spec {
                requires: vec![crate::annotations::Prop::from("True"), rust_prop(None, owner)],
                ensures: vec![],
                invariants: vec![],
            },
            proof: None,
        };
        let model = model(vec![
            result_def("f", &["(x : Std.U32)"], &["x"]),
            helper_def("demo::anneal__f::pre1", "anneal__f.pre1", &["(x : Std.U32)"], &["x"]),
        ]);

        let generated = soundness_module("Demo", &model, &[(0, &spec)], false).unwrap().unwrap();

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

        let generated = soundness_module("Demo", &model, &[], false).unwrap().unwrap();

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
            match_hint: crate::annotation_scan::FunctionMatchHint::Free { module_path: Vec::new() },
            spec: crate::annotations::Spec {
                requires: vec![rust_prop(Some("positive"), owner.clone())],
                ensures: vec![rust_prop(None, owner)],
                invariants: vec![],
            },
            proof: None,
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

        let generated = soundness_module("Demo", &model, &[(0, &spec)], false).unwrap().unwrap();

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
            match_hint: crate::annotation_scan::FunctionMatchHint::Free { module_path: Vec::new() },
            spec: crate::annotations::Spec {
                requires: vec![crate::annotations::Prop::from("x.val > 0")],
                ensures: vec![],
                invariants: vec![],
            },
            proof: None,
        };
        let model = model(vec![result_def("f", &["(x : Std.U32)"], &["x"])]);

        let generated = soundness_module("Demo", &model, &[(0, &spec)], false).unwrap().unwrap();

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
            match_hint: crate::annotation_scan::FunctionMatchHint::Free { module_path: Vec::new() },
            spec: crate::annotations::Spec {
                requires: vec![crate::annotations::Prop::from("y.val > 0")],
                ensures: vec![],
                invariants: vec![],
            },
            proof: None,
        };
        let model = model(vec![
            ResultDef {
                def_id: 0,
                rust_name: "demo::needs_positive".into(),
                name: "needs_positive".into(),
                binders: vec!["(y : Std.U32)".into()],
                args: vec!["y".into()],
                output_type: "Std.U32".into(),
            },
            ResultDef {
                def_id: 0,
                rust_name: "demo::forwards".into(),
                name: "forwards".into(),
                binders: vec!["(x : Std.U32)".into()],
                args: vec!["x".into()],
                output_type: "Std.U32".into(),
            },
        ]);

        let generated =
            soundness_module("Demo", &model, &[(0, &callee_spec)], false).unwrap().unwrap();

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
            match_hint: crate::annotation_scan::FunctionMatchHint::Free { module_path: Vec::new() },
            spec: crate::annotations::Spec {
                requires: vec![],
                ensures: vec![],
                invariants: vec![crate::annotations::Prop::from("x.val <= 10")],
            },
            proof: Some(crate::annotations::Proof::Lean(
                crate::annotations::LeanProof::TacticProof("simp_all".into()),
            )),
        };
        let model = model(vec![result_def("bounded", &["(x : Std.U32)"], &["x"])]);

        let generated = soundness_module("Demo", &model, &[(0, &spec)], false).unwrap().unwrap();

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
            match_hint: crate::annotation_scan::FunctionMatchHint::Free { module_path: Vec::new() },
            spec: crate::annotations::Spec {
                requires: vec![],
                ensures: vec![],
                invariants: vec![crate::annotations::Prop::from("y.val <= 10")],
            },
            proof: Some(crate::annotations::Proof::Lean(
                crate::annotations::LeanProof::TacticProof("simp_all".into()),
            )),
        };
        let model = model(vec![
            ResultDef {
                def_id: 0,
                rust_name: "demo::bounded".into(),
                name: "bounded".into(),
                binders: vec!["(y : Std.U32)".into()],
                args: vec!["y".into()],
                output_type: "Std.U32".into(),
            },
            ResultDef {
                def_id: 0,
                rust_name: "demo::forwards".into(),
                name: "forwards".into(),
                binders: vec!["(x : Std.U32)".into()],
                args: vec!["x".into()],
                output_type: "Std.U32".into(),
            },
        ]);

        let generated =
            soundness_module("Demo", &model, &[(0, &callee_spec)], false).unwrap().unwrap();

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
            match_hint: crate::annotation_scan::FunctionMatchHint::Free { module_path: Vec::new() },
            spec: crate::annotations::Spec {
                requires: vec![],
                ensures: vec![crate::annotations::Prop::from("True")],
                invariants: vec![],
            },
            proof: None,
        };
        let model = model(vec![result_def("f", &["(x : Std.U32)"], &["x"])]);

        let generated = soundness_module("Demo", &model, &[(0, &spec)], false).unwrap().unwrap();
        assert!(generated.lean.contains(" := by\n  unfold f\n  step* by\n"));
    }

    #[test]
    fn annotated_specs_match_aeneas_module_prefixes() {
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
        };
        let model = model(vec![ResultDef {
            def_id: 0,
            rust_name: "demo::gate::allowed".into(),
            name: "gate.allowed".into(),
            binders: vec!["(g : gate.Gate)".into(), "(count : Std.U32)".into()],
            args: vec!["g".into(), "count".into()],
            output_type: "Bool".into(),
        }]);

        let generated = soundness_module("Demo", &model, &[(7, &spec)], false).unwrap().unwrap();

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
        };
        let mut gate = result_def("gate.allowed", &[], &[]);
        gate.rust_name = "demo::gate::allowed".into();
        let mut other = result_def("other.allowed", &[], &[]);
        other.rust_name = "demo::other::allowed".into();
        let model = model(vec![gate, other]);

        let generated = soundness_module("Demo", &model, &[(7, &spec)], false).unwrap().unwrap();

        assert_eq!(generated.matched_specs, vec![7]);
        assert!(generated.lean.contains("theorem gate_allowed_soundness"));
        assert!(!generated.lean.contains("def other_allowed_post1"));
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
                arg_names: vec!["g".into(), "count".into()],
                inputs: vec![u32_ty.clone(), u32_ty.clone()],
                output: u32_ty,
                direct_calls: Vec::new(),
                unsupported_reason: None,
            }],
        };

        let model = model_from_translation_json(&translation, &llbc).unwrap();

        assert_eq!(model.namespace, "demo");
        assert_eq!(model.defs[0].rust_name, "demo::gate::allowed");
        assert_eq!(model.defs[0].name, "gate.allowed");
        assert_eq!(model.defs[0].binders, vec!["(g : Std.U32)", "(count : Std.U32)"]);
        assert_eq!(model.defs[0].application(), "gate.allowed g count");
    }

    #[test]
    fn proof_admissions_are_left_to_lean_semantic_checks() {
        let temp = tempfile::tempdir().unwrap();
        let spec = crate::annotation_scan::FunctionSpec {
            package_manifest_path: temp.path().join("Cargo.toml"),
            path: temp.path().join("src/lib.rs"),
            rust_name: "f".into(),
            match_hint: crate::annotation_scan::FunctionMatchHint::Free { module_path: Vec::new() },
            spec: crate::annotations::Spec::default(),
            proof: Some(crate::annotations::Proof::Lean(
                crate::annotations::LeanProof::TacticProof("sorry".into()),
            )),
        };
        let model = model(vec![result_def("f", &[], &[])]);

        let rejected_by_lean =
            soundness_module("Demo", &model, &[(0, &spec)], false).unwrap().unwrap();
        assert!(rejected_by_lean.lean.contains("  sorry"));
        assert!(rejected_by_lean.lean.contains("set_option warningAsError true"));

        let permitted_by_flag =
            soundness_module("Demo", &model, &[(0, &spec)], true).unwrap().unwrap();
        assert!(permitted_by_flag.lean.contains("  sorry"));
        assert!(!permitted_by_flag.lean.contains("set_option warningAsError true"));

        let mut sorry_ax_spec = spec.clone();
        sorry_ax_spec.proof = Some(crate::annotations::Proof::Lean(
            crate::annotations::LeanProof::TacticProof("exact sorryAx _ true".into()),
        ));
        let sorry_ax =
            soundness_module("Demo", &model, &[(0, &sorry_ax_spec)], false).unwrap().unwrap();
        assert!(sorry_ax.lean.contains("exact sorryAx _ true"));
        assert!(sorry_ax.lean.contains("set_option warningAsError true"));

        let automatic = soundness_module("Demo", &model, &[], false).unwrap().unwrap();
        assert!(automatic.lean.contains("all_goals sorry"));
        assert!(automatic.lean.contains("set_option warningAsError true"));
    }

    #[test]
    fn unsafe_declaration_scanner_ignores_comments_strings_and_longer_identifiers() {
        assert_eq!(lean_unsafe_declaration_keyword("axiom forged : False"), Some("axiom"));
        assert_eq!(lean_unsafe_declaration_keyword("constant forged : False"), Some("constant"));
        assert_eq!(
            lean_unsafe_declaration_keyword(
                "-- axiom\n/- outer /- constant -/ axiom -/\n\
                 def axiomMessage := \"axiom and constant\"\n\
                 exact axiomMessage"
            ),
            None
        );
    }
}
