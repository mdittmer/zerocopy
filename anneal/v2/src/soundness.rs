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
    LeanArg, LeanArgs, LeanAttr, LeanAttrs, LeanProof, Proof, Prop, TheoremDefinition,
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
    types: Vec<TypeDef>,
    skipped: Vec<String>,
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
    rust_name: String,
    name: String,
    binders: Vec<String>,
    args: Vec<String>,
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
        out.push_str(&aux_theorem(aux));
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
            [(def_idx, _)] => {
                matched_specs.push(*spec_idx);
                combined_specs.insert(*def_idx, (*spec).clone());
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

    apply_struct_invariants(model, &struct_invariants_by_type, &mut combined_specs)?;
    combined_specs.retain(|_, spec| {
        !spec.spec.requires.is_empty()
            || !spec.spec.ensures.is_empty()
            || !spec.spec.invariants.is_empty()
    });

    for (def_idx, def) in model.defs.iter().enumerate() {
        out.push_str(&step_soundness_theorem(def, combined_specs.get(&def_idx), allow_sorry)?);
    }

    out.push_str(&format!("end {}\n", model.namespace));
    Ok(Some(GeneratedSoundness { lean: out, matched_specs }))
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
    combined_specs: &mut std::collections::BTreeMap<usize, crate::annotation_scan::FunctionSpec>,
) -> anyhow::Result<()> {
    if struct_invariants_by_type.is_empty() {
        return Ok(());
    }

    for (def_idx, def) in model.defs.iter().enumerate() {
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
                required.extend(struct_invariant_props(invariant, &required_use.value));
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
                ensured.extend(struct_invariant_props(invariant, &ensured_use.value));
            }
        }

        if required.is_empty() && ensured.is_empty() {
            continue;
        }

        let spec = combined_specs.entry(def_idx).or_insert_with(|| {
            let first_invariant = struct_invariants_by_type
                .values()
                .flatten()
                .find(|invariant| function_in_struct_invariant_scope(def, invariant));
            crate::annotation_scan::FunctionSpec {
                package_manifest_path: first_invariant
                    .map(|invariant| invariant.package_manifest_path.clone())
                    .unwrap_or_default(),
                path: first_invariant.map(|invariant| invariant.path.clone()).unwrap_or_default(),
                rust_name: def.rust_basename().to_string(),
                match_hint: crate::annotation_scan::FunctionMatchHint::Basename,
                spec: crate::annotations::Spec::default(),
                proof: None,
            }
        });
        spec.spec.requires.extend(required);
        spec.spec.ensures.extend(ensured);
    }

    Ok(())
}

fn aux_theorem(aux: &crate::annotation_scan::AuxTheorem) -> String {
    let mut out = String::new();
    out.push_str(&format!("/-- Anneal auxiliary theorem from `{}`. -/\n", aux.path.display()));
    match &aux.theorem.definition {
        TheoremDefinition::Lean(definition) => {
            if let Some(attrs) = lean_attrs(&aux.theorem.attrs) {
                out.push_str(&attrs);
                out.push('\n');
            }
            out.push_str("theorem ");
            out.push_str(&aux.theorem.name);
            let args = lean_args(&definition.args);
            if !args.is_empty() {
                out.push(' ');
                out.push_str(&args);
            }
            out.push_str(" : ");
            out.push_str(&definition.ty.0);
            out.push_str(&lean_proof_body(&definition.proof));
            out.push('\n');
        }
    }
    out
}

fn lean_attrs(attrs: &LeanAttrs) -> Option<String> {
    match attrs {
        LeanAttrs::Attrs(attrs) if attrs.is_empty() => None,
        LeanAttrs::Attrs(attrs) => {
            Some(format!("@[{}]", attrs.iter().map(lean_attr).collect::<Vec<_>>().join(", ")))
        }
        LeanAttrs::AttrsString(attrs) if attrs.trim().is_empty() => None,
        LeanAttrs::AttrsString(attrs) => Some(format!("@[{}]", attrs.trim())),
    }
}

fn lean_attr(attr: &LeanAttr) -> &str {
    match attr {
        LeanAttr::AttrString(attr) => attr,
    }
}

fn lean_args(args: &[LeanArgs]) -> String {
    args.iter().map(lean_arg_group).filter(|arg| !arg.is_empty()).collect::<Vec<_>>().join(" ")
}

fn lean_arg_group(args: &LeanArgs) -> String {
    match args {
        LeanArgs::ArgsString(args) => args.trim().to_string(),
        LeanArgs::Args(args) => {
            args.iter().map(lean_arg).filter(|arg| !arg.is_empty()).collect::<Vec<_>>().join(" ")
        }
    }
}

fn lean_arg(arg: &LeanArg) -> String {
    match arg {
        LeanArg::Arg { name, ty } => format!("({name} : {})", ty.0),
        LeanArg::ArgString(arg) => {
            let arg = arg.trim();
            if arg.starts_with(['(', '{', '[']) { arg.to_string() } else { format!("({arg})") }
        }
    }
}

fn lean_proof_body(proof: &LeanProof) -> String {
    match proof {
        LeanProof::TacticProof(proof) => format!(" := by\n{}\n", indent_lean_block(proof)),
        LeanProof::TermProof(proof) => format!(" :=\n{}\n", indent_lean_block(proof)),
    }
}

fn step_soundness_theorem(
    def: &ResultDef,
    spec: Option<&crate::annotation_scan::FunctionSpec>,
    _allow_sorry: bool,
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
        spec.map(|spec| required_condition_hypotheses(&spec.spec)).unwrap_or_default();
    let ensured_props = spec.map(|spec| ensured_condition_refs(&spec.spec)).unwrap_or_default();
    theorem_arguments(&mut out, &def.binders, &required_props);
    out.push_str(" :\n");
    out.push_str("  ");
    out.push_str(&def.application());
    out.push_str(" ⦃ result => ");
    out.push_str(&prop_conjunction(&ensured_props));
    out.push_str(" ⦄");
    let fallback_proof = fallback_step_proof(def);
    out.push_str(&step_proof_body(spec.and_then(|spec| spec.proof.as_ref()), &fallback_proof)?);
    out.push('\n');
    Ok(out)
}

fn fallback_step_proof(def: &ResultDef) -> String {
    let simp_args = "Std.Do.WP.wp, Std.Do.PredTrans.apply";
    format!(
        "unfold {}\nstep* by\n  first | assumption | simp_all [{}] | simp [{}]\nall_goals try assumption\nall_goals try simp_all [{}]\nall_goals try simp [{}]",
        def.name, simp_args, simp_args, simp_args, simp_args
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

fn struct_invariant_props(
    invariant: &crate::annotation_scan::StructInvariant,
    value: &str,
) -> Vec<Prop> {
    invariant
        .invariants
        .iter()
        .map(|prop| Prop::from(replace_lean_identifier(lean_prop_source(prop), "self", value)))
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

fn ensured_props(spec: &crate::annotations::Spec) -> Vec<Prop> {
    let mut props = spec.ensures.clone();
    props.extend(spec.invariants.clone());
    props
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

fn required_condition_hypotheses(spec: &crate::annotations::Spec) -> Vec<(String, String)> {
    let mut refs = spec
        .requires
        .iter()
        .enumerate()
        .map(|(idx, prop)| {
            (format!("required_pre{}'", idx + 1), lean_prop_source(prop).to_string())
        })
        .collect::<Vec<_>>();
    refs.extend(spec.invariants.iter().enumerate().map(|(idx, prop)| {
        (format!("preserved_inv{}'", idx + 1), lean_prop_source(prop).to_string())
    }));
    refs
}

fn ensured_condition_refs(spec: &crate::annotations::Spec) -> Vec<String> {
    ensured_props(spec).iter().map(|prop| lean_prop_source(prop).to_string()).collect()
}

fn prop_conjunction(props: &[String]) -> String {
    match props {
        [] => "True".into(),
        [prop] => format!("({prop})"),
        props => props.iter().map(|prop| format!("({prop})")).collect::<Vec<_>>().join(" ∧ "),
    }
}

fn lean_prop_source(prop: &Prop) -> &str {
    match prop {
        Prop::Lean(lean) => &lean.prop,
    }
}

fn step_proof_body(proof: Option<&Proof>, fallback_proof: &str) -> anyhow::Result<String> {
    match proof {
        Some(Proof::Lean(LeanProof::TacticProof(proof))) => {
            Ok(format!(" := by\n{}\n", indent_lean_block(proof)))
        }
        Some(Proof::Lean(LeanProof::TermProof(proof))) => {
            Ok(format!(" :=\n{}\n", indent_lean_block(proof)))
        }
        None => Ok(format!(" := by\n{}\n", indent_lean_block(fallback_proof))),
    }
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
            rust_name: function.rust_name.clone(),
            name: local_name,
            binders: signature.binders,
            args: signature.args,
            required_structs: signature.required_structs,
            ensured_structs: signature.ensured_structs,
            unsupported_structs: signature.unsupported_structs,
        });
    }

    Ok(SoundnessModel {
        namespace: namespace
            .unwrap_or_else(|| rust_crate_to_lean_namespace(&translation.crate_name)),
        defs,
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
    let unsupported_reason = unsupported_function_generics(fun);
    Some(LlbcFunctionInfo {
        def_id,
        rust_name,
        rust_basename,
        arg_names,
        inputs,
        output,
        unsupported_reason,
    })
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

    Some(LlbcSignatureInfo {
        binders,
        args,
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
            rust_name: format!("demo::{}", name.replace('.', "::")),
            name: name.into(),
            binders: binders.iter().map(|binder| (*binder).into()).collect(),
            args: args.iter().map(|arg| (*arg).into()).collect(),
            required_structs: Vec::new(),
            ensured_structs: Vec::new(),
            unsupported_structs: Vec::new(),
        }
    }

    fn model(defs: Vec<ResultDef>) -> SoundnessModel {
        SoundnessModel { namespace: "demo".into(), defs, types: Vec::new(), skipped: Vec::new() }
    }

    #[test]
    fn emits_default_soundness_module() {
        let model = model(vec![result_def("f", &["(x : Std.U32)"], &["x"])]);

        let generated = soundness_module("Demo", &model, &[], &[], &[], false).unwrap().unwrap();

        assert!(generated.lean.contains("import Demo.Funs"));
        assert!(generated.lean.contains("namespace demo"));
        assert!(generated.lean.contains("@[step]\ntheorem f_soundness\n    (x : Std.U32) :"));
        assert!(generated.lean.contains("f x ⦃ result => True ⦄"));
        assert!(generated.lean.contains(" := by\n  unfold f\n  step* by\n"));
        assert!(!generated.lean.contains("anneal_default_soundness"));
        assert!(!generated.lean.contains("⊢ₛ wp⟦"));
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
        };
        let model = model(vec![result_def("f", &["(x : Std.U32)"], &["x"])]);

        let generated =
            soundness_module("Demo", &model, &[(0, &spec)], &[], &[], false).unwrap().unwrap();

        assert_eq!(generated.matched_specs, vec![0]);
        assert!(generated.lean.contains(
            "@[step]\ntheorem f_soundness\n    (x : Std.U32) :\n  f x ⦃ result => (result.val >= x.val) ⦄"
        ));
        assert!(generated.lean.contains(" := by\n  simp\n"));
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
        };
        let model = model(vec![result_def("f", &["(x : Std.U32)"], &["x"])]);

        let generated =
            soundness_module("Demo", &model, &[(0, &spec)], &[], &[], false).unwrap().unwrap();

        assert!(
            generated
                .lean
                .contains("theorem f_soundness\n    (x : Std.U32)\n    /- -------------------- Anneal hypotheses -------------------- -/\n    (required_pre1' : x.val > 0) :\n  f x ⦃ result => (result.val >= x.val) ⦄")
        );
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
        };
        let model = model(vec![result_def("f", &["(x : Std.U32)"], &["x"])]);

        let generated =
            soundness_module("Demo", &model, &[(0, &spec)], &[], &[], false).unwrap().unwrap();

        assert!(generated.lean.contains(
            "theorem f_soundness\n    (x : Std.U32)\n    /- -------------------- Anneal hypotheses -------------------- -/\n    (required_pre1' : x.val > 0) :\n  f x ⦃ result => True ⦄"
        ));
        assert!(generated.lean.contains(
            " := by\n  unfold f\n  step* by\n    first | assumption | simp_all [Std.Do.WP.wp, Std.Do.PredTrans.apply] | simp [Std.Do.WP.wp, Std.Do.PredTrans.apply]\n"
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
        };
        let model = model(vec![
            ResultDef {
                rust_name: "demo::needs_positive".into(),
                name: "needs_positive".into(),
                binders: vec!["(y : Std.U32)".into()],
                args: vec!["y".into()],
                required_structs: Vec::new(),
                ensured_structs: Vec::new(),
                unsupported_structs: Vec::new(),
            },
            ResultDef {
                rust_name: "demo::forwards".into(),
                name: "forwards".into(),
                binders: vec!["(x : Std.U32)".into()],
                args: vec!["x".into()],
                required_structs: Vec::new(),
                ensured_structs: Vec::new(),
                unsupported_structs: Vec::new(),
            },
        ]);

        let generated = soundness_module("Demo", &model, &[(0, &callee_spec)], &[], &[], false)
            .unwrap()
            .unwrap();

        assert!(generated.lean.contains(
            "theorem needs_positive_soundness\n    (y : Std.U32)\n    /- -------------------- Anneal hypotheses -------------------- -/\n    (required_pre1' : y.val > 0) :\n  needs_positive y ⦃ result => True ⦄"
        ));
        assert!(generated.lean.contains(
            "theorem forwards_soundness\n    (x : Std.U32) :\n  forwards x ⦃ result => True ⦄ := by\n  unfold forwards\n  step*"
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
        };
        let model = model(vec![result_def("bounded", &["(x : Std.U32)"], &["x"])]);

        let generated =
            soundness_module("Demo", &model, &[(0, &spec)], &[], &[], false).unwrap().unwrap();

        assert!(
            generated
                .lean
                .contains("theorem bounded_soundness\n    (x : Std.U32)\n    /- -------------------- Anneal hypotheses -------------------- -/\n    (preserved_inv1' : x.val <= 10) :")
        );
        assert!(generated.lean.contains("bounded x ⦃ result => (x.val <= 10) ⦄"));
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
        };
        let model = model(vec![
            ResultDef {
                rust_name: "demo::bounded".into(),
                name: "bounded".into(),
                binders: vec!["(y : Std.U32)".into()],
                args: vec!["y".into()],
                required_structs: Vec::new(),
                ensured_structs: Vec::new(),
                unsupported_structs: Vec::new(),
            },
            ResultDef {
                rust_name: "demo::forwards".into(),
                name: "forwards".into(),
                binders: vec!["(x : Std.U32)".into()],
                args: vec!["x".into()],
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
                .contains("theorem bounded_soundness\n    (y : Std.U32)\n    /- -------------------- Anneal hypotheses -------------------- -/\n    (preserved_inv1' : y.val <= 10) :")
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
        };
        let model = model(vec![ResultDef {
            rust_name: "demo::gate::allowed".into(),
            name: "gate.allowed".into(),
            binders: vec!["(g : gate.Gate)".into(), "(count : Std.U32)".into()],
            args: vec!["g".into(), "count".into()],
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
        };
        let model = model(vec![ResultDef {
            rust_name: "trait_demo::{impl trait_demo::Bounded for trait_demo::Impl}::bounded"
                .into(),
            name: "Impl.Insts.Trait_demoBounded.bounded".into(),
            binders: vec!["(x : Std.U32)".into()],
            args: vec!["x".into()],
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
            rust_name: "demo::touch".into(),
            name: "touch".into(),
            binders: vec!["(counter : Counter)".into()],
            args: vec!["counter".into()],
            required_structs: vec![StructPropUse { type_id: 0, value: "counter".into() }],
            ensured_structs: vec![StructPropUse { type_id: 0, value: "result".into() }],
            unsupported_structs: Vec::new(),
        }]);
        model.types =
            vec![TypeDef { def_id: 0, rust_name: "demo::Counter".into(), name: "Counter".into() }];

        let generated =
            soundness_module("Demo", &model, &[], &[(0, &invariant)], &[], true).unwrap().unwrap();

        assert!(generated.lean.contains(
            "theorem touch_soundness\n    (counter : Counter)\n    /- -------------------- Anneal hypotheses -------------------- -/\n    (required_pre1' : counter.value.val <= 10) :"
        ));
        assert!(generated.lean.contains("touch counter ⦃ result => (result.value.val <= 10) ⦄"));
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
                rust_name: "demo::touch".into(),
                name: "touch".into(),
                binders: vec!["(counter : Counter)".into()],
                args: vec!["counter".into()],
                required_structs: vec![StructPropUse { type_id: 0, value: "counter".into() }],
                ensured_structs: vec![StructPropUse { type_id: 0, value: "result".into() }],
                unsupported_structs: Vec::new(),
            },
            ResultDef {
                rust_name: "demo::caller".into(),
                name: "caller".into(),
                binders: vec!["(c : Counter)".into()],
                args: vec!["c".into()],
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
            "theorem caller_soundness\n    (c : Counter)\n    /- -------------------- Anneal hypotheses -------------------- -/\n    (required_pre1' : c.value.val <= 10) :\n  caller c ⦃ result => True ⦄"
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
            rust_name: "demo::wrap".into(),
            name: "wrap".into(),
            binders: vec!["(counter : Counter)".into()],
            args: vec!["counter".into()],
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
        assert!(generated.lean.contains("(x : Std.U32) (h : x.val = x.val) : True"));
        assert!(generated.lean.contains(" := by\n  trivial\n"));
    }
}
