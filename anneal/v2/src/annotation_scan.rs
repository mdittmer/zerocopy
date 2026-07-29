// Copyright 2026 The Fuchsia Authors
//
// Licensed under the 2-Clause BSD License <LICENSE-BSD or
// https://opensource.org/license/bsd-2-clause>, Apache License, Version 2.0
// <LICENSE-APACHE or https://www.apache.org/licenses/LICENSE-2.0>, or the MIT
// license <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to
// those terms.

//! Source scanning for Anneal attributes.

use anyhow::Context as _;

use crate::annotations::{
    AnnotationTarget, AttributeData, Proof, RustConditionOwner, Spec, Theorem,
};

/// Anneal attributes found while scanning Rust sources.
#[derive(Debug, Default)]
pub struct AnnotationScan {
    function_specs: Vec<FunctionSpec>,
    struct_invariants: Vec<StructInvariant>,
    aux_theorems: Vec<AuxTheorem>,
    trait_function_specs: Vec<TraitFunctionSpec>,
    impl_functions: Vec<ImplFunction>,
    unsupported: Vec<UnsupportedAnnotation>,
    expansion_expectations: Vec<ExpansionExpectation>,
}

impl AnnotationScan {
    /// Function-level postcondition specs that the current generator supports.
    pub fn function_specs(&self) -> &[FunctionSpec] {
        &self.function_specs
    }

    /// Struct-level invariants that the current generator supports.
    pub fn struct_invariants(&self) -> &[StructInvariant] {
        &self.struct_invariants
    }

    /// Auxiliary Lean theorem declarations from soundness-domain annotations.
    pub fn aux_theorems(&self) -> &[AuxTheorem] {
        &self.aux_theorems
    }

    /// Fails if soundness-domain annotations were found but cannot yet be interpreted.
    pub fn reject_unsupported(&self) -> anyhow::Result<()> {
        if self.unsupported.is_empty() {
            return Ok(());
        }

        let mut details = self
            .unsupported
            .iter()
            .take(8)
            .map(|item| {
                let name =
                    item.rust_name.as_ref().map(|name| format!(" `{name}`")).unwrap_or_default();
                format!("{}{} ({:?}): {}", item.path.display(), name, item.target, item.reason)
            })
            .collect::<Vec<_>>();
        if self.unsupported.len() > details.len() {
            details.push(format!("... and {} more", self.unsupported.len() - details.len()));
        }

        anyhow::bail!(
            "found {} unsupported Anneal soundness annotation item(s):\n{}",
            self.unsupported.len(),
            details.join("\n")
        )
    }

    fn expand_trait_function_specs(&mut self) {
        for implementation in &self.impl_functions {
            let matching_trait_specs = self
                .trait_function_specs
                .iter()
                .filter(|spec| {
                    implementation.package_manifest_path == spec.package_manifest_path
                        && implementation.trait_name.as_deref() == Some(spec.trait_name.as_str())
                        && implementation.method_name == spec.method_name
                })
                .collect::<Vec<_>>();

            if matching_trait_specs.is_empty() {
                if implementation.has_soundness {
                    self.function_specs.push(function_spec_from_impl(
                        implementation,
                        implementation.spec.clone(),
                        implementation.proof.clone(),
                    ));
                }
                continue;
            }

            for trait_spec in matching_trait_specs {
                let mut spec = trait_spec.spec.clone();
                spec.requires.extend(implementation.spec.requires.clone());
                spec.ensures.extend(implementation.spec.ensures.clone());
                spec.invariants.extend(implementation.spec.invariants.clone());
                let proof = implementation.proof.clone().or_else(|| trait_spec.proof.clone());
                self.function_specs.push(function_spec_from_impl(implementation, spec, proof));
            }
        }
    }
}

fn function_spec_from_impl(
    implementation: &ImplFunction,
    spec: Spec,
    proof: Option<Proof>,
) -> FunctionSpec {
    let match_hint = match (&implementation.trait_name, &implementation.self_ty) {
        (Some(trait_name), Some(self_ty)) => FunctionMatchHint::TraitImpl {
            trait_name: trait_name.clone(),
            self_ty: self_ty.clone(),
            method_name: implementation.method_name.clone(),
        },
        _ => FunctionMatchHint::Basename,
    };
    FunctionSpec {
        package_manifest_path: implementation.package_manifest_path.clone(),
        path: implementation.path.clone(),
        rust_name: implementation.method_name.clone(),
        match_hint,
        spec,
        proof,
        rust_theorem: false,
    }
}

#[derive(Clone, Debug)]
pub struct FunctionSpec {
    pub package_manifest_path: std::path::PathBuf,
    pub path: std::path::PathBuf,
    pub rust_name: String,
    pub match_hint: FunctionMatchHint,
    pub spec: Spec,
    pub proof: Option<Proof>,
    pub rust_theorem: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FunctionMatchHint {
    /// Legacy/fallback matching for synthesized specs without source module identity.
    Basename,
    /// A free function identified by its module path within the selected crate.
    Free {
        module_path: Vec<String>,
    },
    TraitImpl {
        trait_name: String,
        self_ty: String,
        method_name: String,
    },
}

#[derive(Debug)]
pub struct StructInvariant {
    pub package_manifest_path: std::path::PathBuf,
    pub path: std::path::PathBuf,
    pub rust_name: String,
    pub module_path: Vec<String>,
    pub invariants: Vec<crate::annotations::Prop>,
}

#[derive(Debug)]
pub struct AuxTheorem {
    pub package_manifest_path: std::path::PathBuf,
    pub path: std::path::PathBuf,
    pub theorem: Theorem,
}

#[derive(Debug)]
struct UnsupportedAnnotation {
    path: std::path::PathBuf,
    rust_name: Option<String>,
    target: AnnotationTarget,
    reason: String,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum ExpansionTarget {
    Function,
    Type,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum ExpansionOwner {
    Named {
        target: ExpansionTarget,
        path: Vec<String>,
    },
    TraitImplFunction {
        module_path: Vec<String>,
        trait_name: String,
        self_ty: String,
        method: String,
    },
    OpaqueImplFunction {
        module_path: Vec<String>,
        method: String,
    },
}

#[derive(Clone, Debug)]
struct ExpansionExpectation {
    package_manifest_path: std::path::PathBuf,
    crate_root_path: std::path::PathBuf,
    path: std::path::PathBuf,
    owner: ExpansionOwner,
    marker: String,
}

#[derive(Debug)]
struct TraitFunctionSpec {
    package_manifest_path: std::path::PathBuf,
    trait_name: String,
    method_name: String,
    spec: Spec,
    proof: Option<Proof>,
}

#[derive(Debug)]
struct ImplFunction {
    package_manifest_path: std::path::PathBuf,
    path: std::path::PathBuf,
    trait_name: Option<String>,
    self_ty: Option<String>,
    method_name: String,
    spec: Spec,
    proof: Option<Proof>,
    has_soundness: bool,
}

#[derive(Clone, Debug)]
enum FunctionContext {
    Free,
    Trait { trait_name: String, has_default: bool, enclosing_generics: bool },
    Impl { trait_name: Option<String>, self_ty: Option<String> },
}

/// Scans Rust sources for the selected Anneal artifacts.
pub fn scan_packages(
    packages: &[crate::scanner::AnnealArtifact],
) -> anyhow::Result<AnnotationScan> {
    let mut scan = AnnotationScan::default();

    // One Cargo target can expose multiple crate types, so deduplicate identical
    // crate roots rather than whole packages. Distinct bins, examples, tests,
    // and libraries in the same package are distinct module graphs.
    let mut visited_roots = std::collections::BTreeSet::new();
    for artifact in packages {
        let source_path = artifact.src_path.as_deref().ok_or_else(|| {
            anyhow::anyhow!(
                "selected Anneal target `{}` in {} has no Rust source path",
                artifact.name.target_name,
                artifact.manifest_path.display()
            )
        })?;
        let source_path = std::fs::canonicalize(source_path).with_context(|| {
            format!("failed to canonicalize selected Rust source {}", source_path.display())
        })?;
        if !visited_roots.insert((artifact.manifest_path.clone(), source_path.clone())) {
            continue;
        }
        let mut traversal =
            ModuleTraversal { crate_root_path: Some(source_path.clone()), ..Default::default() };
        scan_module_file(
            &mut scan,
            &artifact.manifest_path,
            &source_path,
            &[],
            true,
            false,
            &mut traversal,
        )?;
    }
    scan.expand_trait_function_specs();
    Ok(scan)
}

/// Reconciles the unexpanded source scan with metadata emitted by the proc
/// macro and retained in LLBC.
///
/// Rust name resolution and macro expansion can make an attribute visible to
/// rustc while hiding it from a `syn` walk (for example a renamed proc macro,
/// `macro_rules!`, or `include!`).  Silently omitting such a theorem or
/// precondition would be unsound, so unsupported spellings fail closed.
pub fn validate_expansion_markers(
    roots: &crate::resolve::LockedRoots,
    packages: &[crate::scanner::AnnealArtifact],
    scan: &AnnotationScan,
) -> anyhow::Result<()> {
    type Key = (ExpansionOwner, String);

    for artifact in packages {
        let llbc_path = artifact.llbc_path(roots);
        if !llbc_path.exists() {
            anyhow::bail!(
                "Charon completed without producing expansion metadata at {} for target `{}`",
                llbc_path.display(),
                artifact.name.target_name
            );
        }
        let crate_data: charon_lib::export::CrateData = serde_json::from_reader(
            std::fs::File::open(&llbc_path)
                .with_context(|| format!("failed to open {}", llbc_path.display()))?,
        )
        .with_context(|| format!("failed to parse {}", llbc_path.display()))?;
        let artifact_root = artifact.src_path.as_deref().ok_or_else(|| {
            anyhow::anyhow!(
                "selected Anneal target `{}` has no Rust source path",
                artifact.name.target_name
            )
        })?;
        let artifact_root = std::fs::canonicalize(artifact_root).with_context(|| {
            format!("failed to canonicalize selected Rust source {}", artifact_root.display())
        })?;

        let mut expected = std::collections::BTreeMap::<Key, Vec<std::path::PathBuf>>::new();
        for expectation in scan.expansion_expectations.iter().filter(|expectation| {
            expectation.package_manifest_path == artifact.manifest_path
                && expectation.crate_root_path == artifact_root
        }) {
            expected
                .entry((expectation.owner.clone(), expectation.marker.clone()))
                .or_default()
                .push(expectation.path.clone());
        }

        let mut expanded = std::collections::BTreeMap::<Key, usize>::new();
        for function in
            crate_data.translated.fun_decls.iter().filter(|function| function.item_meta.is_local)
        {
            // A provided trait method also has a FunDecl for its default body;
            // its marker is reconciled through TraitDecl.methods below.
            if matches!(function.src, charon_lib::ast::ItemSource::TraitDecl { .. }) {
                continue;
            }
            let Some(owner) = charon_function_owner(function, &crate_data.translated) else {
                continue;
            };
            for marker in expansion_markers(&function.item_meta.attr_info) {
                *expanded.entry((owner.clone(), marker)).or_default() += 1;
            }
        }
        for trait_decl in crate_data
            .translated
            .trait_decls
            .iter()
            .filter(|trait_decl| trait_decl.item_meta.is_local)
        {
            for method in &trait_decl.methods {
                let method = &method.skip_binder;
                let Some(owner) =
                    charon_named_owner(ExpansionTarget::Function, &method.item_meta.name)
                else {
                    continue;
                };
                for marker in expansion_markers(&method.item_meta.attr_info) {
                    *expanded.entry((owner.clone(), marker)).or_default() += 1;
                }
            }
        }
        for ty in crate_data.translated.type_decls.iter().filter(|ty| ty.item_meta.is_local) {
            let Some(owner) = charon_named_owner(ExpansionTarget::Type, &ty.item_meta.name) else {
                continue;
            };
            for marker in expansion_markers(&ty.item_meta.attr_info) {
                *expanded.entry((owner.clone(), marker)).or_default() += 1;
            }
        }

        let expected_counts = expected
            .iter()
            .map(|(key, paths)| (key.clone(), paths.len()))
            .collect::<std::collections::BTreeMap<_, _>>();
        if expected_counts == expanded {
            continue;
        }

        let mut details = Vec::new();
        for (key @ (owner, _), expanded_count) in &expanded {
            let expected_count = expected_counts.get(key).copied().unwrap_or(0);
            if *expanded_count > expected_count {
                details.push(format!(
                    "rustc expanded {} Anneal attribute(s) on {} that \
                     the source scanner did not observe",
                    expanded_count - expected_count,
                    expansion_owner_display(owner)
                ));
            }
        }
        for (key @ (owner, _), paths) in &expected {
            let expanded_count = expanded.get(key).copied().unwrap_or(0);
            if paths.len() > expanded_count {
                details.push(format!(
                    "{} directly spelled Anneal attribute(s) on {} in {} \
                     did not leave expansion metadata",
                    paths.len() - expanded_count,
                    expansion_owner_display(owner),
                    paths[expanded_count].display()
                ));
            }
        }
        anyhow::bail!(
            "Anneal's original-source scan disagrees with rustc macro expansion for target `{}`:\n\
             {}\n\
             Use a directly spelled `#[anneal(...)]` or \
             `#[anneal_macros::anneal(...)]`; renamed, `macro_rules!`-generated, and \
             `include!`-generated Anneal attributes are rejected until their expanded-to-original \
             provenance can be mapped precisely.",
            artifact.name.target_name,
            details.join("\n")
        );
    }
    Ok(())
}

fn expansion_markers(attr_info: &charon_lib::ast::AttrInfo) -> Vec<String> {
    attr_info
        .attributes
        .iter()
        .filter_map(|attr| match attr {
            charon_lib::ast::Attribute::DocComment(comment)
                if crate::annotations::is_expansion_marker(comment) =>
            {
                Some(comment.clone())
            }
            _ => None,
        })
        .collect()
}

fn charon_function_owner(
    function: &charon_lib::ast::FunDecl,
    translated: &charon_lib::ast::TranslatedCrate,
) -> Option<ExpansionOwner> {
    if let charon_lib::ast::ItemSource::TraitImpl { trait_ref, .. } = &function.src {
        let method = charon_item_basename(&function.item_meta.name)?;
        let mut module_path = charon_ident_path(&function.item_meta.name)?;
        module_path.pop();
        let trait_name = translated
            .trait_decls
            .get(trait_ref.id)
            .and_then(|trait_decl| charon_item_basename(&trait_decl.item_meta.name));
        let self_ty =
            trait_ref.self_ty(translated).and_then(|ty| charon_type_basename(ty, translated));
        if let (Some(trait_name), Some(self_ty)) = (trait_name, self_ty) {
            return Some(ExpansionOwner::TraitImplFunction {
                module_path,
                trait_name,
                self_ty,
                method,
            });
        }
        return Some(ExpansionOwner::OpaqueImplFunction { module_path, method });
    }
    if function
        .item_meta
        .name
        .name
        .iter()
        .any(|element| matches!(element, charon_lib::ast::PathElem::Impl(_)))
    {
        let method = charon_item_basename(&function.item_meta.name)?;
        let mut module_path = charon_ident_path(&function.item_meta.name)?;
        module_path.pop();
        return Some(ExpansionOwner::OpaqueImplFunction { module_path, method });
    }
    charon_named_owner(ExpansionTarget::Function, &function.item_meta.name)
}

fn charon_named_owner(
    target: ExpansionTarget,
    name: &charon_lib::ast::Name,
) -> Option<ExpansionOwner> {
    let path = charon_ident_path(name)?;
    Some(ExpansionOwner::Named { target, path })
}

fn charon_ident_path(name: &charon_lib::ast::Name) -> Option<Vec<String>> {
    let mut path = name
        .name
        .iter()
        .filter_map(|element| match element {
            charon_lib::ast::PathElem::Ident(identifier, _) => Some(identifier.clone()),
            _ => None,
        })
        .collect::<Vec<_>>();
    if path.is_empty() {
        return None;
    }
    path.remove(0);
    Some(path)
}

fn charon_type_basename(
    ty: &charon_lib::ast::Ty,
    translated: &charon_lib::ast::TranslatedCrate,
) -> Option<String> {
    match ty.kind() {
        charon_lib::ast::TyKind::Adt(decl) => match decl.id {
            charon_lib::ast::TypeId::Adt(type_id) => translated
                .type_decls
                .get(type_id)
                .and_then(|ty| charon_item_basename(&ty.item_meta.name)),
            _ => None,
        },
        charon_lib::ast::TyKind::Literal(literal) => Some(
            match literal {
                charon_lib::ast::LiteralTy::Bool => "bool",
                charon_lib::ast::LiteralTy::Char => "char",
                charon_lib::ast::LiteralTy::UInt(value) => match value {
                    charon_lib::ast::UIntTy::Usize => "usize",
                    charon_lib::ast::UIntTy::U8 => "u8",
                    charon_lib::ast::UIntTy::U16 => "u16",
                    charon_lib::ast::UIntTy::U32 => "u32",
                    charon_lib::ast::UIntTy::U64 => "u64",
                    charon_lib::ast::UIntTy::U128 => "u128",
                },
                charon_lib::ast::LiteralTy::Int(value) => match value {
                    charon_lib::ast::IntTy::Isize => "isize",
                    charon_lib::ast::IntTy::I8 => "i8",
                    charon_lib::ast::IntTy::I16 => "i16",
                    charon_lib::ast::IntTy::I32 => "i32",
                    charon_lib::ast::IntTy::I64 => "i64",
                    charon_lib::ast::IntTy::I128 => "i128",
                },
                charon_lib::ast::LiteralTy::Float(_) => return None,
            }
            .to_string(),
        ),
        charon_lib::ast::TyKind::Pattern(inner, _) => charon_type_basename(inner, translated),
        _ => None,
    }
}

fn charon_item_basename(name: &charon_lib::ast::Name) -> Option<String> {
    name.name.iter().rev().find_map(|element| match element {
        charon_lib::ast::PathElem::Ident(identifier, _) => Some(identifier.clone()),
        _ => None,
    })
}

fn expansion_owner_display(owner: &ExpansionOwner) -> String {
    match owner {
        ExpansionOwner::Named { target, path } => {
            format!("{target:?} `{}`", path.join("::"))
        }
        ExpansionOwner::TraitImplFunction { module_path, trait_name, self_ty, method } => {
            let prefix = if module_path.is_empty() {
                String::new()
            } else {
                format!("{}::", module_path.join("::"))
            };
            format!("trait impl Function `{prefix}<{self_ty} as {trait_name}>::{method}`")
        }
        ExpansionOwner::OpaqueImplFunction { module_path, method } => {
            let prefix = if module_path.is_empty() {
                String::new()
            } else {
                format!("{}::", module_path.join("::"))
            };
            format!("impl Function `{prefix}{method}`")
        }
    }
}

#[derive(Default)]
struct ModuleTraversal {
    crate_root_path: Option<std::path::PathBuf>,
    visited: std::collections::BTreeSet<(std::path::PathBuf, Vec<String>)>,
    active_files: std::collections::BTreeSet<std::path::PathBuf>,
}

fn scan_module_file(
    scan: &mut AnnotationScan,
    package_manifest_path: &std::path::Path,
    path: &std::path::Path,
    module_path: &[String],
    is_crate_root: bool,
    conditionally_compiled: bool,
    traversal: &mut ModuleTraversal,
) -> anyhow::Result<()> {
    let path = std::fs::canonicalize(path)
        .with_context(|| format!("failed to canonicalize Rust module {}", path.display()))?;
    if !traversal.visited.insert((path.clone(), module_path.to_vec())) {
        return Ok(());
    }
    if !traversal.active_files.insert(path.clone()) {
        anyhow::bail!("Rust module graph contains a source cycle through {}", path.display());
    }

    let result = (|| {
        let source = std::fs::read_to_string(&path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        let file = syn::parse_file(&source)
            .with_context(|| format!("failed to parse Rust source {}", path.display()))?;
        let source_dir = path.parent().ok_or_else(|| {
            anyhow::anyhow!("Rust module source {} has no parent directory", path.display())
        })?;
        let module_dir =
            if is_crate_root || path.file_name().is_some_and(|file_name| file_name == "mod.rs") {
                source_dir.to_path_buf()
            } else {
                let stem = path.file_stem().and_then(|stem| stem.to_str()).ok_or_else(|| {
                    anyhow::anyhow!("Rust module source {} has no UTF-8 file stem", path.display())
                })?;
                source_dir.join(stem)
            };
        scan_items(
            scan,
            package_manifest_path,
            &path,
            module_path,
            &module_dir,
            source_dir,
            &file.items,
            conditionally_compiled,
            traversal,
        )
    })();
    traversal.active_files.remove(&path);
    result
}

fn scan_items(
    scan: &mut AnnotationScan,
    package_manifest_path: &std::path::Path,
    path: &std::path::Path,
    module_path: &[String],
    module_dir: &std::path::Path,
    path_attr_dir: &std::path::Path,
    items: &[syn::Item],
    conditionally_compiled: bool,
    traversal: &mut ModuleTraversal,
) -> anyhow::Result<()> {
    let crate_root_path = traversal
        .crate_root_path
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("internal error: module traversal has no crate root"))?
        .to_path_buf();
    for item in items {
        reject_translation_exclude(path, item_attrs(item))?;
        match item {
            syn::Item::Fn(item) => record_function_attrs(
                scan,
                package_manifest_path,
                &crate_root_path,
                path,
                module_path,
                AnnotationTarget::Function,
                FunctionContext::Free,
                item.sig.ident.to_string(),
                &item.sig,
                matches!(item.vis, syn::Visibility::Public(_)),
                conditionally_compiled || has_cfg_control(&item.attrs),
                &item.attrs,
            )?,
            syn::Item::Struct(item) => {
                record_struct_attrs(
                    scan,
                    package_manifest_path,
                    &crate_root_path,
                    path,
                    module_path,
                    item,
                    conditionally_compiled || has_cfg_control(&item.attrs),
                )?;
                for field in &item.fields {
                    record_attrs(
                        scan,
                        package_manifest_path,
                        path,
                        AnnotationTarget::Other,
                        None,
                        &field.attrs,
                    )?;
                }
            }
            syn::Item::Trait(item) => {
                let trait_is_conditional = conditionally_compiled || has_cfg_control(&item.attrs);
                record_attrs(
                    scan,
                    package_manifest_path,
                    path,
                    AnnotationTarget::Trait,
                    Some(item.ident.to_string()),
                    &item.attrs,
                )?;
                for trait_item in &item.items {
                    if let syn::TraitItem::Fn(function) = trait_item {
                        reject_translation_exclude(path, &function.attrs)?;
                        record_function_attrs(
                            scan,
                            package_manifest_path,
                            &crate_root_path,
                            path,
                            module_path,
                            AnnotationTarget::TraitFunction,
                            FunctionContext::Trait {
                                trait_name: item.ident.to_string(),
                                has_default: function.default.is_some(),
                                enclosing_generics: !item.generics.params.is_empty()
                                    || item.generics.where_clause.is_some(),
                            },
                            function.sig.ident.to_string(),
                            &function.sig,
                            false,
                            trait_is_conditional || has_cfg_control(&function.attrs),
                            &function.attrs,
                        )?;
                    }
                }
            }
            syn::Item::Impl(item) => {
                let impl_is_conditional = conditionally_compiled || has_cfg_control(&item.attrs);
                let trait_name =
                    item.trait_.as_ref().and_then(|(_, path, _)| path_last_ident(path));
                let self_ty = type_last_ident(&item.self_ty);
                record_attrs(
                    scan,
                    package_manifest_path,
                    path,
                    AnnotationTarget::Impl,
                    None,
                    &item.attrs,
                )?;
                for impl_item in &item.items {
                    if let syn::ImplItem::Fn(function) = impl_item {
                        reject_translation_exclude(path, &function.attrs)?;
                        record_function_attrs(
                            scan,
                            package_manifest_path,
                            &crate_root_path,
                            path,
                            module_path,
                            AnnotationTarget::ImplFunction,
                            FunctionContext::Impl {
                                trait_name: trait_name.clone(),
                                self_ty: self_ty.clone(),
                            },
                            function.sig.ident.to_string(),
                            &function.sig,
                            matches!(function.vis, syn::Visibility::Public(_)),
                            impl_is_conditional || has_cfg_control(&function.attrs),
                            &function.attrs,
                        )?;
                    }
                }
            }
            syn::Item::Mod(item) => {
                let module_is_conditional = conditionally_compiled || has_cfg_control(&item.attrs);
                record_attrs(
                    scan,
                    package_manifest_path,
                    path,
                    AnnotationTarget::Other,
                    Some(item.ident.to_string()),
                    &item.attrs,
                )?;
                if let Some((_, items)) = &item.content {
                    let mut child_module_path = module_path.to_vec();
                    child_module_path.push(item.ident.to_string());
                    let child_dir = if let Some(path_attr) = module_path_attribute(&item.attrs)? {
                        path_attr_dir.join(path_attr)
                    } else {
                        module_dir.join(item.ident.to_string())
                    };
                    scan_items(
                        scan,
                        package_manifest_path,
                        path,
                        &child_module_path,
                        &child_dir,
                        &child_dir,
                        items,
                        module_is_conditional,
                        traversal,
                    )?;
                } else if let Some(child_path) =
                    external_module_path(item, module_dir, path_attr_dir)?
                {
                    let mut child_module_path = module_path.to_vec();
                    child_module_path.push(item.ident.to_string());
                    scan_module_file(
                        scan,
                        package_manifest_path,
                        &child_path,
                        &child_module_path,
                        false,
                        module_is_conditional,
                        traversal,
                    )?;
                }
            }
            item => {
                record_attrs(
                    scan,
                    package_manifest_path,
                    path,
                    AnnotationTarget::Other,
                    None,
                    item_attrs(item),
                )?;
            }
        }
    }
    Ok(())
}

fn external_module_path(
    item: &syn::ItemMod,
    module_dir: &std::path::Path,
    path_attr_dir: &std::path::Path,
) -> anyhow::Result<Option<std::path::PathBuf>> {
    if let Some(path) = module_path_attribute(&item.attrs)? {
        let path = path_attr_dir.join(path);
        return Ok(path.is_file().then_some(path));
    }

    let name = item.ident.to_string();
    let flat = module_dir.join(format!("{name}.rs"));
    let nested = module_dir.join(&name).join("mod.rs");
    match (flat.is_file(), nested.is_file()) {
        (true, true) => anyhow::bail!(
            "Rust module `{name}` has both candidate source files {} and {}",
            flat.display(),
            nested.display()
        ),
        (true, false) => Ok(Some(flat)),
        (false, true) => Ok(Some(nested)),
        (false, false) => Ok(None),
    }
}

fn module_path_attribute(attrs: &[syn::Attribute]) -> anyhow::Result<Option<std::path::PathBuf>> {
    let mut paths = attrs.iter().filter(|attr| attr.path().is_ident("path"));
    let Some(attr) = paths.next() else {
        return Ok(None);
    };
    if paths.next().is_some() {
        anyhow::bail!("Rust module has multiple `path` attributes");
    }
    let syn::Meta::NameValue(name_value) = &attr.meta else {
        anyhow::bail!("Rust module `path` attribute must have the form `#[path = \"...\"]`");
    };
    let syn::Expr::Lit(expr) = &name_value.value else {
        anyhow::bail!("Rust module `path` attribute must be a string literal");
    };
    let syn::Lit::Str(path) = &expr.lit else {
        anyhow::bail!("Rust module `path` attribute must be a string literal");
    };
    let path = std::path::PathBuf::from(path.value());
    if path.as_os_str().is_empty() {
        anyhow::bail!("Rust module `path` attribute cannot be empty");
    }
    Ok(Some(path))
}

fn item_attrs(item: &syn::Item) -> &[syn::Attribute] {
    match item {
        syn::Item::Fn(item) => &item.attrs,
        syn::Item::Impl(item) => &item.attrs,
        syn::Item::Mod(item) => &item.attrs,
        syn::Item::Struct(item) => &item.attrs,
        syn::Item::Trait(item) => &item.attrs,
        syn::Item::Const(item) => &item.attrs,
        syn::Item::Enum(item) => &item.attrs,
        syn::Item::ExternCrate(item) => &item.attrs,
        syn::Item::ForeignMod(item) => &item.attrs,
        syn::Item::Macro(item) => &item.attrs,
        syn::Item::Static(item) => &item.attrs,
        syn::Item::Type(item) => &item.attrs,
        syn::Item::Union(item) => &item.attrs,
        syn::Item::Use(item) => &item.attrs,
        _ => &[],
    }
}

fn reject_translation_exclude(
    path: &std::path::Path,
    attrs: &[syn::Attribute],
) -> anyhow::Result<()> {
    let excluded = attrs.iter().any(|attr| {
        let segments = attr
            .path()
            .segments
            .iter()
            .map(|segment| segment.ident.to_string())
            .collect::<Vec<_>>();
        matches!(segments.as_slice(), [tool, exclude]
            if (tool == "charon" || tool == "aeneas") && exclude == "exclude")
    });
    if excluded {
        anyhow::bail!(
            "`#[charon::exclude]`/`#[aeneas::exclude]` on local items in {} is incompatible \
             with Anneal expansion reconciliation and is not supported",
            path.display()
        );
    }
    Ok(())
}

fn has_cfg_control(attrs: &[syn::Attribute]) -> bool {
    attrs.iter().any(|attr| {
        let path = attr.path();
        path.leading_colon.is_none()
            && path.segments.len() == 1
            && matches!(
                path.segments.first().map(|segment| segment.ident.to_string()).as_deref(),
                Some("cfg" | "cfg_attr")
            )
    })
}

fn path_last_ident(path: &syn::Path) -> Option<String> {
    path.segments.last().map(|segment| segment.ident.to_string())
}

fn type_last_ident(ty: &syn::Type) -> Option<String> {
    match ty {
        syn::Type::Path(path) => path_last_ident(&path.path),
        _ => None,
    }
}

fn record_attrs(
    scan: &mut AnnotationScan,
    package_manifest_path: &std::path::Path,
    path: &std::path::Path,
    target: AnnotationTarget,
    rust_name: Option<String>,
    attrs: &[syn::Attribute],
) -> anyhow::Result<()> {
    let attrs = crate::annotations::parse_attributes_for_target(attrs, target)
        .with_context(|| format!("failed to parse Anneal attributes in {}", path.display()))?;
    for attr in attrs.into_iter().filter(is_soundness_attr) {
        match attr.data {
            AttributeData::Theorem(theorem) => {
                scan.aux_theorems.push(AuxTheorem {
                    package_manifest_path: package_manifest_path.to_path_buf(),
                    path: path.to_path_buf(),
                    theorem,
                });
            }
            AttributeData::RustTheorem | AttributeData::Spec(_) | AttributeData::Proof(_) => {
                scan.unsupported.push(UnsupportedAnnotation {
                    path: path.to_path_buf(),
                    rust_name: rust_name.clone(),
                    target,
                    reason: format!("soundness annotations on {target:?} are not implemented yet"),
                });
            }
        }
    }
    Ok(())
}

fn record_struct_attrs(
    scan: &mut AnnotationScan,
    package_manifest_path: &std::path::Path,
    crate_root_path: &std::path::Path,
    path: &std::path::Path,
    module_path: &[String],
    item: &syn::ItemStruct,
    conditionally_compiled: bool,
) -> anyhow::Result<()> {
    let mut owner_path = module_path.to_vec();
    owner_path.push(item.ident.to_string());
    record_expansion_expectations(
        scan,
        package_manifest_path,
        crate_root_path,
        path,
        ExpansionOwner::Named { target: ExpansionTarget::Type, path: owner_path },
        conditionally_compiled,
        &item.attrs,
    )?;
    let attrs =
        crate::annotations::parse_attributes_for_target(&item.attrs, AnnotationTarget::Struct)
            .with_context(|| format!("failed to parse Anneal attributes in {}", path.display()))?;
    for attr in attrs.into_iter().filter(is_soundness_attr) {
        match attr.data {
            AttributeData::Spec(spec) => {
                if spec.invariants.is_empty() {
                    push_unsupported(
                        scan,
                        path,
                        AnnotationTarget::Struct,
                        Some(item.ident.to_string()),
                        "struct soundness specs require at least one invariant",
                    );
                    continue;
                }
                if spec_has_rust_props(&spec)
                    && (!item.generics.params.is_empty() || item.generics.where_clause.is_some())
                {
                    push_unsupported(
                        scan,
                        path,
                        AnnotationTarget::Struct,
                        Some(item.ident.to_string()),
                        "Rust-authored Anneal invariants on generic structs are not supported yet",
                    );
                    continue;
                }
                if let Some(reason) = unsupported_struct_invariant_shape(item) {
                    push_unsupported(
                        scan,
                        path,
                        AnnotationTarget::Struct,
                        Some(item.ident.to_string()),
                        reason,
                    );
                    continue;
                }
                scan.struct_invariants.push(StructInvariant {
                    package_manifest_path: package_manifest_path.to_path_buf(),
                    path: path.to_path_buf(),
                    rust_name: item.ident.to_string(),
                    module_path: module_path.to_vec(),
                    invariants: with_prop_owner(
                        spec.invariants,
                        RustConditionOwner::Struct {
                            module_path: module_path.to_vec(),
                            struct_name: item.ident.to_string(),
                        },
                    ),
                });
            }
            AttributeData::Theorem(theorem) => {
                scan.aux_theorems.push(AuxTheorem {
                    package_manifest_path: package_manifest_path.to_path_buf(),
                    path: path.to_path_buf(),
                    theorem,
                });
            }
            AttributeData::RustTheorem => {
                push_unsupported(
                    scan,
                    path,
                    AnnotationTarget::Struct,
                    Some(item.ident.to_string()),
                    "bare `theorem` markers are supported only on public free functions",
                );
            }
            AttributeData::Proof(_) => {
                push_unsupported(
                    scan,
                    path,
                    AnnotationTarget::Struct,
                    Some(item.ident.to_string()),
                    "proof annotations on structs are not implemented yet; attach proofs to functions that establish or preserve the invariant",
                );
            }
        }
    }
    Ok(())
}

fn unsupported_struct_invariant_shape(item: &syn::ItemStruct) -> Option<&'static str> {
    let syn::Fields::Named(fields) = &item.fields else {
        return Some(
            "struct invariants are currently supported only for structs with named fields",
        );
    };
    if fields.named.iter().any(|field| !matches!(field.vis, syn::Visibility::Inherited)) {
        return Some("struct invariants are currently supported only when all fields are private");
    }
    None
}

fn record_function_attrs(
    scan: &mut AnnotationScan,
    package_manifest_path: &std::path::Path,
    crate_root_path: &std::path::Path,
    path: &std::path::Path,
    module_path: &[String],
    target: AnnotationTarget,
    context: FunctionContext,
    rust_name: String,
    sig: &syn::Signature,
    is_public: bool,
    conditionally_compiled: bool,
    attrs: &[syn::Attribute],
) -> anyhow::Result<()> {
    let expansion_owner = match &context {
        FunctionContext::Free => {
            let mut path = module_path.to_vec();
            path.push(rust_name.clone());
            ExpansionOwner::Named { target: ExpansionTarget::Function, path }
        }
        FunctionContext::Trait { trait_name, .. } => {
            let mut path = module_path.to_vec();
            path.extend([trait_name.clone(), rust_name.clone()]);
            ExpansionOwner::Named { target: ExpansionTarget::Function, path }
        }
        FunctionContext::Impl { trait_name: Some(trait_name), self_ty: Some(self_ty) } => {
            ExpansionOwner::TraitImplFunction {
                module_path: module_path.to_vec(),
                trait_name: trait_name.clone(),
                self_ty: self_ty.clone(),
                method: rust_name.clone(),
            }
        }
        FunctionContext::Impl { .. } => ExpansionOwner::OpaqueImplFunction {
            module_path: module_path.to_vec(),
            method: rust_name.clone(),
        },
    };
    record_expansion_expectations(
        scan,
        package_manifest_path,
        crate_root_path,
        path,
        expansion_owner,
        conditionally_compiled,
        attrs,
    )?;
    let attrs = crate::annotations::parse_attributes_for_target(attrs, target)
        .with_context(|| format!("failed to parse Anneal attributes in {}", path.display()))?;
    let mut spec = Spec::default();
    let mut proof = None;
    let mut rust_theorem = false;
    let mut has_soundness = false;
    let mut supported = true;

    for attr in attrs.into_iter().filter(is_soundness_attr) {
        has_soundness = true;
        match attr.data {
            AttributeData::Spec(partial) => {
                spec.requires.extend(partial.requires);
                spec.ensures.extend(partial.ensures);
                spec.invariants.extend(partial.invariants);
            }
            AttributeData::Proof(value) => {
                if proof.replace(value).is_some() {
                    push_unsupported(
                        scan,
                        path,
                        target,
                        Some(rust_name.clone()),
                        "multiple proof annotations for one function are not supported yet",
                    );
                    supported = false;
                }
            }
            AttributeData::RustTheorem => {
                if rust_theorem {
                    push_unsupported(
                        scan,
                        path,
                        target,
                        Some(rust_name.clone()),
                        "multiple bare `theorem` markers on one function are not supported",
                    );
                    supported = false;
                }
                rust_theorem = true;
            }
            AttributeData::Theorem(theorem) => {
                scan.aux_theorems.push(AuxTheorem {
                    package_manifest_path: package_manifest_path.to_path_buf(),
                    path: path.to_path_buf(),
                    theorem,
                });
            }
        }
    }

    if rust_theorem && let Some(reason) = unsupported_rust_theorem_shape(&context, sig, is_public) {
        push_unsupported(scan, path, target, Some(rust_name.clone()), reason);
        supported = false;
    }

    let owner = match &context {
        FunctionContext::Free => RustConditionOwner::Function {
            module_path: module_path.to_vec(),
            function: rust_name.clone(),
        },
        FunctionContext::Trait { trait_name, .. } => RustConditionOwner::TraitMethod {
            module_path: module_path.to_vec(),
            trait_name: trait_name.clone(),
            method: rust_name.clone(),
        },
        FunctionContext::Impl { trait_name, self_ty } => RustConditionOwner::ImplMethod {
            module_path: module_path.to_vec(),
            trait_name: trait_name.clone(),
            self_ty: self_ty.clone(),
            method: rust_name.clone(),
        },
    };
    set_spec_prop_owner(&mut spec, owner);

    if spec_has_rust_props(&spec) {
        let reason = match &context {
            FunctionContext::Trait { has_default: true, .. } => Some(
                "Rust-authored specs on trait methods with default bodies are not supported yet; \
                 the attribute macro cannot distinguish their expansion context from a free \
                 function",
            ),
            FunctionContext::Impl { .. } => Some(
                "Rust-authored specs directly on impl methods are not supported yet; put the \
                 contract on the trait method so it applies to every impl",
            ),
            FunctionContext::Trait { enclosing_generics: true, .. } => {
                Some("Rust-authored specs on methods of generic traits are not supported yet")
            }
            FunctionContext::Free | FunctionContext::Trait { .. } => {
                unsupported_rust_condition_shape(sig)
            }
        };
        if let Some(reason) = reason {
            push_unsupported(scan, path, target, Some(rust_name.clone()), reason);
            supported = false;
        }
    }

    if matches!(context, FunctionContext::Impl { .. }) {
        let FunctionContext::Impl { trait_name, self_ty } = context.clone() else {
            unreachable!();
        };
        scan.impl_functions.push(ImplFunction {
            package_manifest_path: package_manifest_path.to_path_buf(),
            path: path.to_path_buf(),
            trait_name,
            self_ty,
            method_name: rust_name.clone(),
            spec: spec.clone(),
            proof: proof.clone(),
            has_soundness,
        });
    }

    if !has_soundness {
        return Ok(());
    }

    let proof_only_trait_impl =
        matches!(context, FunctionContext::Impl { trait_name: Some(_), .. })
            && proof.is_some()
            && spec.requires.is_empty()
            && spec.ensures.is_empty()
            && spec.invariants.is_empty();
    let proof_only_free = matches!(context, FunctionContext::Free)
        && proof.is_some()
        && spec.requires.is_empty()
        && spec.ensures.is_empty()
        && spec.invariants.is_empty();
    if spec.requires.is_empty()
        && spec.ensures.is_empty()
        && spec.invariants.is_empty()
        && !proof_only_trait_impl
        && !proof_only_free
        && !rust_theorem
    {
        if proof.is_some() {
            push_unsupported(
                scan,
                path,
                target,
                Some(rust_name),
                "proof annotations require a supported soundness obligation",
            );
        }
        return Ok(());
    }
    if proof_only_trait_impl {
        return Ok(());
    }

    if !supported {
        return Ok(());
    }

    match context {
        FunctionContext::Free => {
            scan.function_specs.push(FunctionSpec {
                package_manifest_path: package_manifest_path.to_path_buf(),
                path: path.to_path_buf(),
                rust_name,
                match_hint: FunctionMatchHint::Free { module_path: module_path.to_vec() },
                spec,
                proof,
                rust_theorem,
            });
        }
        FunctionContext::Trait { trait_name, .. } => {
            scan.trait_function_specs.push(TraitFunctionSpec {
                package_manifest_path: package_manifest_path.to_path_buf(),
                trait_name,
                method_name: rust_name,
                spec,
                proof,
            });
        }
        FunctionContext::Impl { trait_name: None, .. } => {
            push_unsupported(
                scan,
                path,
                target,
                Some(rust_name),
                "soundness annotations on inherent impl functions are not implemented yet",
            );
        }
        FunctionContext::Impl { .. } => {
            // Trait impl methods are expanded after all files have been scanned,
            // once their trait method specs are known.
        }
    }
    Ok(())
}

fn record_expansion_expectations(
    scan: &mut AnnotationScan,
    package_manifest_path: &std::path::Path,
    crate_root_path: &std::path::Path,
    path: &std::path::Path,
    owner: ExpansionOwner,
    conditionally_compiled: bool,
    attrs: &[syn::Attribute],
) -> anyhow::Result<()> {
    for attr in attrs {
        if let Some(marker) = crate::annotations::expansion_marker_for_attribute(attr)
            .with_context(|| {
                format!("failed to identify Anneal expansion marker in {}", path.display())
            })?
        {
            if conditionally_compiled {
                anyhow::bail!(
                    "conditionally compiled Anneal attribute on {} in {} is not supported; \
                     expansion reconciliation requires one unconditional original Rust item",
                    expansion_owner_display(&owner),
                    path.display()
                );
            }
            scan.expansion_expectations.push(ExpansionExpectation {
                package_manifest_path: package_manifest_path.to_path_buf(),
                crate_root_path: crate_root_path.to_path_buf(),
                path: path.to_path_buf(),
                owner: owner.clone(),
                marker,
            });
        }
    }
    Ok(())
}

fn spec_has_rust_props(spec: &Spec) -> bool {
    spec.requires
        .iter()
        .chain(&spec.ensures)
        .chain(&spec.invariants)
        .any(|prop| matches!(prop, crate::annotations::Prop::Rust(_)))
}

fn unsupported_rust_theorem_shape(
    context: &FunctionContext,
    sig: &syn::Signature,
    is_public: bool,
) -> Option<&'static str> {
    if !matches!(context, FunctionContext::Free) {
        return Some("bare `theorem` markers are supported only on public free functions");
    }
    if !is_public {
        return Some("Rust theorem functions must use unrestricted `pub` visibility");
    }
    if !sig.generics.params.is_empty() || sig.generics.where_clause.is_some() {
        return Some("Rust theorem functions must be concrete and cannot have generic parameters");
    }
    if signature_contains_impl_trait(sig) {
        return Some("Rust theorem functions must be concrete and cannot use `impl Trait`");
    }
    if sig.asyncness.is_some()
        || sig.constness.is_some()
        || sig.abi.is_some()
        || sig.variadic.is_some()
    {
        return Some("Rust theorem functions cannot be async, const, extern, or variadic");
    }
    if !returns_plain_bool(sig) {
        return Some("Rust theorem functions must return exactly `bool`");
    }
    if signature_contains_mutable_reference(sig) {
        return Some("Rust theorem functions cannot take or return mutable references");
    }
    None
}

fn unsupported_rust_condition_shape(sig: &syn::Signature) -> Option<&'static str> {
    if !sig.generics.params.is_empty() || sig.generics.where_clause.is_some() {
        return Some("Rust-authored Anneal conditions on generic functions are not supported yet");
    }
    if signature_contains_impl_trait(sig) {
        return Some(
            "Rust-authored Anneal conditions using `impl Trait` are not supported; use a concrete type",
        );
    }
    if sig.asyncness.is_some()
        || sig.constness.is_some()
        || sig.abi.is_some()
        || sig.variadic.is_some()
    {
        return Some(
            "Rust-authored Anneal conditions on async, const, extern, or variadic functions are not supported yet",
        );
    }
    if signature_contains_mutable_reference(sig) {
        return Some(
            "Rust-authored Anneal conditions with mutable-reference inputs or outputs are not supported yet",
        );
    }
    None
}

fn returns_plain_bool(sig: &syn::Signature) -> bool {
    let syn::ReturnType::Type(_, ty) = &sig.output else {
        return false;
    };
    let syn::Type::Path(path) = &**ty else {
        return false;
    };
    path.qself.is_none()
        && path.path.leading_colon.is_none()
        && path.path.segments.len() == 1
        && path.path.segments[0].ident == "bool"
        && matches!(path.path.segments[0].arguments, syn::PathArguments::None)
}

fn signature_contains_mutable_reference(sig: &syn::Signature) -> bool {
    sig.inputs.iter().any(|arg| match arg {
        syn::FnArg::Receiver(receiver) => type_contains_mutable_reference(&receiver.ty),
        syn::FnArg::Typed(arg) => type_contains_mutable_reference(&arg.ty),
    }) || match &sig.output {
        syn::ReturnType::Default => false,
        syn::ReturnType::Type(_, ty) => type_contains_mutable_reference(ty),
    }
}

fn signature_contains_impl_trait(sig: &syn::Signature) -> bool {
    sig.inputs.iter().any(|arg| match arg {
        syn::FnArg::Receiver(receiver) => type_contains_impl_trait(&receiver.ty),
        syn::FnArg::Typed(arg) => type_contains_impl_trait(&arg.ty),
    }) || match &sig.output {
        syn::ReturnType::Default => false,
        syn::ReturnType::Type(_, ty) => type_contains_impl_trait(ty),
    }
}

fn type_contains_impl_trait(ty: &syn::Type) -> bool {
    match ty {
        syn::Type::ImplTrait(_) => true,
        syn::Type::Array(array) => type_contains_impl_trait(&array.elem),
        syn::Type::BareFn(function) => {
            function.inputs.iter().any(|arg| type_contains_impl_trait(&arg.ty))
                || match &function.output {
                    syn::ReturnType::Default => false,
                    syn::ReturnType::Type(_, ty) => type_contains_impl_trait(ty),
                }
        }
        syn::Type::Group(group) => type_contains_impl_trait(&group.elem),
        syn::Type::Paren(paren) => type_contains_impl_trait(&paren.elem),
        syn::Type::Path(path) => {
            path.qself.as_ref().is_some_and(|qself| type_contains_impl_trait(&qself.ty))
                || path
                    .path
                    .segments
                    .iter()
                    .any(|segment| path_arguments_contain_impl_trait(&segment.arguments))
        }
        syn::Type::Ptr(pointer) => type_contains_impl_trait(&pointer.elem),
        syn::Type::Reference(reference) => type_contains_impl_trait(&reference.elem),
        syn::Type::Slice(slice) => type_contains_impl_trait(&slice.elem),
        syn::Type::Tuple(tuple) => tuple.elems.iter().any(type_contains_impl_trait),
        _ => false,
    }
}

fn path_arguments_contain_impl_trait(arguments: &syn::PathArguments) -> bool {
    match arguments {
        syn::PathArguments::None => false,
        syn::PathArguments::AngleBracketed(arguments) => {
            arguments.args.iter().any(|argument| match argument {
                syn::GenericArgument::Type(ty) => type_contains_impl_trait(ty),
                syn::GenericArgument::AssocType(assoc) => type_contains_impl_trait(&assoc.ty),
                _ => false,
            })
        }
        syn::PathArguments::Parenthesized(arguments) => {
            arguments.inputs.iter().any(type_contains_impl_trait)
                || match &arguments.output {
                    syn::ReturnType::Default => false,
                    syn::ReturnType::Type(_, ty) => type_contains_impl_trait(ty),
                }
        }
    }
}

fn type_contains_mutable_reference(ty: &syn::Type) -> bool {
    match ty {
        syn::Type::Reference(reference) => {
            reference.mutability.is_some() || type_contains_mutable_reference(&reference.elem)
        }
        syn::Type::Array(array) => type_contains_mutable_reference(&array.elem),
        syn::Type::BareFn(function) => {
            function.inputs.iter().any(|arg| type_contains_mutable_reference(&arg.ty))
                || match &function.output {
                    syn::ReturnType::Default => false,
                    syn::ReturnType::Type(_, ty) => type_contains_mutable_reference(ty),
                }
        }
        syn::Type::Group(group) => type_contains_mutable_reference(&group.elem),
        syn::Type::Paren(paren) => type_contains_mutable_reference(&paren.elem),
        syn::Type::Path(path) => {
            path.qself.as_ref().is_some_and(|qself| type_contains_mutable_reference(&qself.ty))
                || path
                    .path
                    .segments
                    .iter()
                    .any(|segment| path_arguments_contain_mutable_reference(&segment.arguments))
        }
        syn::Type::Ptr(pointer) => type_contains_mutable_reference(&pointer.elem),
        syn::Type::Slice(slice) => type_contains_mutable_reference(&slice.elem),
        syn::Type::Tuple(tuple) => tuple.elems.iter().any(type_contains_mutable_reference),
        _ => false,
    }
}

fn path_arguments_contain_mutable_reference(arguments: &syn::PathArguments) -> bool {
    match arguments {
        syn::PathArguments::None => false,
        syn::PathArguments::AngleBracketed(arguments) => {
            arguments.args.iter().any(|argument| match argument {
                syn::GenericArgument::Type(ty) => type_contains_mutable_reference(ty),
                syn::GenericArgument::AssocType(assoc) => {
                    type_contains_mutable_reference(&assoc.ty)
                }
                _ => false,
            })
        }
        syn::PathArguments::Parenthesized(arguments) => {
            arguments.inputs.iter().any(type_contains_mutable_reference)
                || match &arguments.output {
                    syn::ReturnType::Default => false,
                    syn::ReturnType::Type(_, ty) => type_contains_mutable_reference(ty),
                }
        }
    }
}

fn set_spec_prop_owner(spec: &mut Spec, owner: RustConditionOwner) {
    for prop in spec.requires.iter_mut().chain(&mut spec.ensures).chain(&mut spec.invariants) {
        if let crate::annotations::Prop::Rust(rust) = prop {
            rust.owner = Some(owner.clone());
        }
    }
}

fn with_prop_owner(
    mut props: Vec<crate::annotations::Prop>,
    owner: RustConditionOwner,
) -> Vec<crate::annotations::Prop> {
    for prop in &mut props {
        if let crate::annotations::Prop::Rust(rust) = prop {
            rust.owner = Some(owner.clone());
        }
    }
    props
}

fn push_unsupported(
    scan: &mut AnnotationScan,
    path: &std::path::Path,
    target: AnnotationTarget,
    rust_name: Option<String>,
    reason: impl Into<String>,
) {
    scan.unsupported.push(UnsupportedAnnotation {
        path: path.to_path_buf(),
        rust_name,
        target,
        reason: reason.into(),
    });
}

fn is_soundness_attr(attr: &crate::annotations::Attribute) -> bool {
    match &attr.data {
        AttributeData::Spec(_)
        | AttributeData::RustTheorem
        | AttributeData::Theorem(_)
        | AttributeData::Proof(_) => attr.domains.iter().any(|domain| domain == "soundness"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scan_test_file(
        scan: &mut AnnotationScan,
        package_manifest_path: &std::path::Path,
        path: &std::path::Path,
    ) -> anyhow::Result<()> {
        let mut traversal = ModuleTraversal {
            crate_root_path: Some(std::fs::canonicalize(path)?),
            ..Default::default()
        };
        scan_module_file(scan, package_manifest_path, path, &[], true, false, &mut traversal)
    }

    #[test]
    fn scan_file_finds_soundness_function_attr() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("lib.rs");
        std::fs::write(
            &path,
            r#"
            #[anneal(spec(ensures = "True"))]
            pub fn f(x: u32) -> u32 { x }
            "#,
        )
        .unwrap();

        let mut scan = AnnotationScan::default();
        scan_test_file(&mut scan, &temp.path().join("Cargo.toml"), &path).unwrap();

        assert_eq!(scan.function_specs.len(), 1);
        assert_eq!(scan.function_specs[0].rust_name, "f");
        assert_eq!(scan.function_specs[0].spec.ensures.len(), 1);
        scan.reject_unsupported().unwrap();
    }

    #[test]
    fn scan_file_finds_canonical_qualified_macro_attribute() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("lib.rs");
        std::fs::write(
            &path,
            r#"
            #[anneal_macros::anneal(spec(requires(prop = flag)))]
            pub fn qualified(flag: bool) -> bool { flag }
            "#,
        )
        .unwrap();

        let mut scan = AnnotationScan::default();
        scan_test_file(&mut scan, &temp.path().join("Cargo.toml"), &path).unwrap();

        assert_eq!(scan.function_specs.len(), 1);
        assert_eq!(scan.function_specs[0].rust_name, "qualified");
        assert_eq!(
            scan.function_specs[0].match_hint,
            FunctionMatchHint::Free { module_path: Vec::new() }
        );
        scan.reject_unsupported().unwrap();
    }

    #[test]
    fn scan_file_ignores_non_soundness_domain() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("lib.rs");
        std::fs::write(
            &path,
            r#"
            #[anneal(spec(ensures = "True", domain = "documentation"))]
            pub fn f(x: u32) -> u32 { x }
            "#,
        )
        .unwrap();

        let mut scan = AnnotationScan::default();
        scan_test_file(&mut scan, &temp.path().join("Cargo.toml"), &path).unwrap();

        assert!(scan.function_specs.is_empty());
        scan.reject_unsupported().unwrap();
    }

    #[test]
    fn scan_file_reports_schema_errors_with_file_context() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("lib.rs");
        std::fs::write(
            &path,
            r#"
            #[anneal(spec(requires = "x > 0"))]
            pub struct Bad {
                x: u32,
            }
            "#,
        )
        .unwrap();

        let mut scan = AnnotationScan::default();
        let err = scan_test_file(&mut scan, &temp.path().join("Cargo.toml"), &path)
            .unwrap_err()
            .to_string();

        assert!(err.contains("failed to parse Anneal attributes"));
    }

    #[test]
    fn scan_file_records_preconditions() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("lib.rs");
        std::fs::write(
            &path,
            r#"
            #[anneal(spec(
                requires(name = "positive", prop = x > 0),
                ensures(name = "identity", prop = ret == x),
            ))]
            pub fn f(x: u32) -> u32 { x }
            "#,
        )
        .unwrap();

        let mut scan = AnnotationScan::default();
        scan_test_file(&mut scan, &temp.path().join("Cargo.toml"), &path).unwrap();

        assert_eq!(scan.function_specs.len(), 1);
        assert_eq!(scan.function_specs[0].spec.requires.len(), 1);
        assert_eq!(scan.function_specs[0].spec.ensures.len(), 1);
        let crate::annotations::Prop::Rust(requires) = &scan.function_specs[0].spec.requires[0]
        else {
            panic!("expected Rust precondition");
        };
        assert_eq!(
            requires.owner,
            Some(RustConditionOwner::Function { module_path: Vec::new(), function: "f".into() })
        );
        scan.reject_unsupported().unwrap();
    }

    #[test]
    fn scan_file_rejects_cfg_attr_wrapped_specs() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("lib.rs");
        std::fs::write(
            &path,
            r#"
            #[cfg_attr(anneal, anneal(spec(requires = "x.val > 0", ensures = "True")))]
            pub fn f(x: u32) -> u32 { x }
            "#,
        )
        .unwrap();

        let mut scan = AnnotationScan::default();
        let err = format!(
            "{:#}",
            scan_test_file(&mut scan, &temp.path().join("Cargo.toml"), &path).unwrap_err()
        );

        assert!(err.contains("cfg_attr-wrapped Anneal attributes are not supported"));
    }

    #[test]
    fn scan_file_rejects_cfg_controlled_annotations_and_enclosing_modules() {
        for source in [
            r#"
                #[cfg(any())]
                #[anneal(theorem)]
                pub fn hidden() -> bool { false }
            "#,
            r#"
                #[cfg(any())]
                mod conditional {
                    #[anneal(theorem)]
                    pub fn hidden() -> bool { false }
                }
            "#,
        ] {
            let temp = tempfile::tempdir().unwrap();
            let path = temp.path().join("lib.rs");
            std::fs::write(&path, source).unwrap();
            let mut scan = AnnotationScan::default();

            let error =
                scan_test_file(&mut scan, &temp.path().join("Cargo.toml"), &path).unwrap_err();

            assert!(error.to_string().contains("conditionally compiled Anneal attribute"));
        }
    }

    #[test]
    fn scan_file_rejects_translation_exclusion_as_unreconcilable() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("lib.rs");
        std::fs::write(
            &path,
            r#"
                #[charon::exclude]
                pub fn hidden() -> bool { false }
            "#,
        )
        .unwrap();
        let mut scan = AnnotationScan::default();

        let error = scan_test_file(&mut scan, &temp.path().join("Cargo.toml"), &path).unwrap_err();

        assert!(error.to_string().contains("incompatible with Anneal expansion reconciliation"));
    }

    #[test]
    fn scan_file_records_precondition_only_specs() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("lib.rs");
        std::fs::write(
            &path,
            r#"
            #[anneal(spec(requires = "x.val > 0"))]
            pub fn f(x: u32) -> u32 { x }
            "#,
        )
        .unwrap();

        let mut scan = AnnotationScan::default();
        scan_test_file(&mut scan, &temp.path().join("Cargo.toml"), &path).unwrap();

        assert_eq!(scan.function_specs.len(), 1);
        assert_eq!(scan.function_specs[0].spec.requires.len(), 1);
        scan.reject_unsupported().unwrap();
    }

    #[test]
    fn scan_packages_scans_selected_example_source_only() {
        let temp = tempfile::tempdir().unwrap();
        let manifest_path = temp.path().join("Cargo.toml");
        let examples = temp.path().join("examples");
        std::fs::create_dir_all(&examples).unwrap();
        std::fs::write(&manifest_path, "").unwrap();
        let selected = examples.join("selected.rs");
        std::fs::write(
            &selected,
            r#"
            #[anneal(spec(requires = "flag = true"))]
            pub fn selected(flag: bool) -> bool { flag }

            fn main() { let _ = selected(true); }
            "#,
        )
        .unwrap();
        std::fs::write(
            examples.join("sibling.rs"),
            r#"
            #[anneal(spec(invariant = "self.value.val <= 10"))]
            pub struct Counter {
                value: u32,
            }
            "#,
        )
        .unwrap();

        let artifact = crate::scanner::AnnealArtifact {
            name: crate::resolve::AnnealTargetName {
                package_name: serde_json::from_str("\"scan-demo\"").unwrap(),
                target_name: "selected".into(),
                kind: crate::resolve::AnnealTargetKind::Example,
            },
            target_kind: crate::resolve::AnnealTargetKind::Example,
            manifest_path,
            src_path: Some(selected),
        };
        let scan = scan_packages(&[artifact]).unwrap();

        assert_eq!(scan.function_specs.len(), 1);
        assert_eq!(scan.function_specs[0].rust_name, "selected");
        assert!(scan.struct_invariants.is_empty());
        scan.reject_unsupported().unwrap();
    }

    #[test]
    fn selected_library_scans_its_module_graph_not_unrelated_bins_or_tests() {
        let temp = tempfile::tempdir().unwrap();
        let manifest_path = temp.path().join("Cargo.toml");
        let src = temp.path().join("src");
        let tests = temp.path().join("tests");
        std::fs::create_dir_all(src.join("bin")).unwrap();
        std::fs::create_dir_all(&tests).unwrap();
        std::fs::write(&manifest_path, "").unwrap();
        let selected = src.join("lib.rs");
        std::fs::write(
            &selected,
            r#"
            #[anneal(spec(requires(prop = flag)))]
            pub fn selected_library(flag: bool) -> bool { flag }
            "#,
        )
        .unwrap();
        std::fs::write(
            src.join("bin/unrelated.rs"),
            r#"
            #[anneal(spec(requires(prop = flag)))]
            pub fn unrelated_bin(flag: bool) -> bool { flag }
            "#,
        )
        .unwrap();
        std::fs::write(
            tests.join("unrelated.rs"),
            r#"
            #[anneal(spec(requires(prop = flag)))]
            pub fn unrelated_test(flag: bool) -> bool { flag }
            "#,
        )
        .unwrap();

        let artifact = crate::scanner::AnnealArtifact {
            name: crate::resolve::AnnealTargetName {
                package_name: serde_json::from_str("\"scan-demo\"").unwrap(),
                target_name: "scan-demo".into(),
                kind: crate::resolve::AnnealTargetKind::Lib,
            },
            target_kind: crate::resolve::AnnealTargetKind::Lib,
            manifest_path,
            src_path: Some(selected),
        };
        let scan = scan_packages(&[artifact]).unwrap();

        assert_eq!(scan.function_specs.len(), 1);
        assert_eq!(scan.function_specs[0].rust_name, "selected_library");
        assert!(!scan.function_specs.iter().any(|spec| spec.rust_name.starts_with("unrelated")));
        scan.reject_unsupported().unwrap();
    }

    #[test]
    fn selected_bin_source_is_crate_root_and_unrelated_bins_are_not_scanned() {
        let temp = tempfile::tempdir().unwrap();
        let manifest_path = temp.path().join("Cargo.toml");
        let bin_dir = temp.path().join("src/bin");
        std::fs::create_dir_all(&bin_dir).unwrap();
        std::fs::write(&manifest_path, "").unwrap();
        let selected = bin_dir.join("tool.rs");
        std::fs::write(
            &selected,
            r#"
            mod support;

            #[anneal(spec(requires(prop = flag)))]
            pub fn selected_root(flag: bool) -> bool { flag }
            "#,
        )
        .unwrap();
        std::fs::write(
            bin_dir.join("support.rs"),
            r#"
            #[anneal(spec(requires(prop = flag)))]
            pub fn selected_support(flag: bool) -> bool { flag }
            "#,
        )
        .unwrap();
        std::fs::write(
            bin_dir.join("unrelated.rs"),
            r#"
            #[anneal(spec(requires(prop = flag)))]
            pub fn unrelated(flag: bool) -> bool { flag }
            "#,
        )
        .unwrap();

        let artifact = crate::scanner::AnnealArtifact {
            name: crate::resolve::AnnealTargetName {
                package_name: serde_json::from_str("\"scan-demo\"").unwrap(),
                target_name: "tool".into(),
                kind: crate::resolve::AnnealTargetKind::Bin,
            },
            target_kind: crate::resolve::AnnealTargetKind::Bin,
            manifest_path,
            src_path: Some(selected),
        };
        let scan = scan_packages(&[artifact]).unwrap();

        assert_eq!(scan.function_specs.len(), 2);
        let root =
            scan.function_specs.iter().find(|spec| spec.rust_name == "selected_root").unwrap();
        let crate::annotations::Prop::Rust(root_prop) = &root.spec.requires[0] else {
            panic!("expected Rust precondition");
        };
        assert_eq!(
            root_prop.owner,
            Some(RustConditionOwner::Function {
                module_path: Vec::new(),
                function: "selected_root".into(),
            })
        );
        let support =
            scan.function_specs.iter().find(|spec| spec.rust_name == "selected_support").unwrap();
        let crate::annotations::Prop::Rust(support_prop) = &support.spec.requires[0] else {
            panic!("expected Rust precondition");
        };
        assert_eq!(
            support_prop.owner,
            Some(RustConditionOwner::Function {
                module_path: vec!["support".into()],
                function: "selected_support".into(),
            })
        );
        assert!(!scan.function_specs.iter().any(|spec| spec.rust_name == "unrelated"));
        scan.reject_unsupported().unwrap();
    }

    #[test]
    fn scan_file_follows_external_modules_with_declared_module_paths() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("lib.rs");
        std::fs::write(&root, "mod api;\n").unwrap();
        std::fs::write(
            temp.path().join("api.rs"),
            r#"
            mod nested;

            #[anneal(spec(requires(prop = flag)))]
            pub fn endpoint(flag: bool) -> bool { flag }
            "#,
        )
        .unwrap();
        std::fs::create_dir_all(temp.path().join("api")).unwrap();
        std::fs::write(
            temp.path().join("api/nested.rs"),
            r#"
            #[anneal(spec(requires(prop = flag)))]
            pub fn nested_endpoint(flag: bool) -> bool { flag }
            "#,
        )
        .unwrap();

        let mut scan = AnnotationScan::default();
        scan_test_file(&mut scan, &temp.path().join("Cargo.toml"), &root).unwrap();

        assert_eq!(scan.function_specs.len(), 2);
        for (name, expected_module) in
            [("endpoint", vec!["api"]), ("nested_endpoint", vec!["api", "nested"])]
        {
            let spec = scan.function_specs.iter().find(|spec| spec.rust_name == name).unwrap();
            let crate::annotations::Prop::Rust(prop) = &spec.spec.requires[0] else {
                panic!("expected Rust precondition");
            };
            let Some(RustConditionOwner::Function { module_path, .. }) = &prop.owner else {
                panic!("expected function condition owner");
            };
            assert_eq!(module_path, &expected_module);
        }
        scan.reject_unsupported().unwrap();
    }

    #[test]
    fn scan_file_honors_path_attribute_but_uses_declared_module_path() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("lib.rs");
        let layout = temp.path().join("layout");
        std::fs::create_dir_all(layout.join("implementation")).unwrap();
        std::fs::write(&root, "#[path = \"layout/implementation.rs\"]\nmod declared;\n").unwrap();
        std::fs::write(
            layout.join("implementation.rs"),
            r#"
            mod nested;

            #[anneal(spec(requires(prop = flag)))]
            pub fn redirected(flag: bool) -> bool { flag }
            "#,
        )
        .unwrap();
        std::fs::write(
            layout.join("implementation/nested.rs"),
            r#"
            #[anneal(spec(requires(prop = flag)))]
            pub fn redirected_nested(flag: bool) -> bool { flag }
            "#,
        )
        .unwrap();

        let mut scan = AnnotationScan::default();
        scan_test_file(&mut scan, &temp.path().join("Cargo.toml"), &root).unwrap();

        for (name, expected_module) in
            [("redirected", vec!["declared"]), ("redirected_nested", vec!["declared", "nested"])]
        {
            let spec = scan.function_specs.iter().find(|spec| spec.rust_name == name).unwrap();
            let crate::annotations::Prop::Rust(prop) = &spec.spec.requires[0] else {
                panic!("expected Rust precondition");
            };
            let Some(RustConditionOwner::Function { module_path, .. }) = &prop.owner else {
                panic!("expected function condition owner");
            };
            assert_eq!(module_path, &expected_module);
        }
        scan.reject_unsupported().unwrap();
    }

    #[test]
    fn scan_file_records_function_invariants() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("lib.rs");
        std::fs::write(
            &path,
            r#"
            #[anneal(spec(invariant = "x.val <= 10"))]
            pub fn f(x: u32) -> u32 { x }
            "#,
        )
        .unwrap();

        let mut scan = AnnotationScan::default();
        scan_test_file(&mut scan, &temp.path().join("Cargo.toml"), &path).unwrap();

        assert_eq!(scan.function_specs.len(), 1);
        assert_eq!(scan.function_specs[0].spec.invariants.len(), 1);
        scan.reject_unsupported().unwrap();
    }

    #[test]
    fn scan_file_records_struct_invariants_for_private_named_fields() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("lib.rs");
        std::fs::write(
            &path,
            r#"
            mod gate {
                #[anneal(spec(invariant = "self.value.val <= 10"))]
                pub struct Counter {
                    value: u32,
                }
            }
            "#,
        )
        .unwrap();

        let mut scan = AnnotationScan::default();
        scan_test_file(&mut scan, &temp.path().join("Cargo.toml"), &path).unwrap();

        assert_eq!(scan.struct_invariants.len(), 1);
        assert_eq!(scan.struct_invariants[0].rust_name, "Counter");
        assert_eq!(scan.struct_invariants[0].module_path, vec!["gate"]);
        assert_eq!(scan.struct_invariants[0].invariants.len(), 1);
        scan.reject_unsupported().unwrap();
    }

    #[test]
    fn scan_file_rejects_struct_invariants_for_public_fields() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("lib.rs");
        std::fs::write(
            &path,
            r#"
            #[anneal(spec(invariant = "self.value.val <= 10"))]
            pub struct Counter {
                pub value: u32,
            }
            "#,
        )
        .unwrap();

        let mut scan = AnnotationScan::default();
        scan_test_file(&mut scan, &temp.path().join("Cargo.toml"), &path).unwrap();

        let err = scan.reject_unsupported().unwrap_err().to_string();
        assert!(err.contains("all fields are private"));
    }

    #[test]
    fn scan_file_expands_trait_function_specs_to_impls() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("lib.rs");
        std::fs::write(
            &path,
            r#"
            pub trait Bounded {
                #[anneal(spec(requires = "x.val <= 10"))]
                fn bounded(x: u32) -> u32;
            }

            pub struct Impl;

            impl Bounded for Impl {
                #[anneal(proof = "simp_all")]
                fn bounded(x: u32) -> u32 { x }
            }
            "#,
        )
        .unwrap();

        let mut scan = AnnotationScan::default();
        scan_test_file(&mut scan, &temp.path().join("Cargo.toml"), &path).unwrap();
        scan.expand_trait_function_specs();

        assert_eq!(scan.function_specs.len(), 1);
        assert_eq!(scan.function_specs[0].rust_name, "bounded");
        assert_eq!(scan.function_specs[0].spec.requires.len(), 1);
        assert!(scan.function_specs[0].proof.is_some());
        assert_eq!(
            scan.function_specs[0].match_hint,
            FunctionMatchHint::TraitImpl {
                trait_name: "Bounded".into(),
                self_ty: "Impl".into(),
                method_name: "bounded".into(),
            }
        );
        scan.reject_unsupported().unwrap();
    }

    #[test]
    fn scan_file_records_function_proof() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("lib.rs");
        std::fs::write(
            &path,
            r#"
            #[anneal(spec(ensures = "True"), proof = "simp")]
            pub fn f(x: u32) -> u32 { x }
            "#,
        )
        .unwrap();

        let mut scan = AnnotationScan::default();
        scan_test_file(&mut scan, &temp.path().join("Cargo.toml"), &path).unwrap();

        assert_eq!(scan.function_specs.len(), 1);
        assert!(scan.function_specs[0].proof.is_some());
        scan.reject_unsupported().unwrap();
    }

    #[test]
    fn scan_file_records_rust_theorem_with_auto_proof() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("lib.rs");
        std::fs::write(
            &path,
            r#"
            #[anneal(theorem, spec(ensures = "x.val > 0"), proof(auto))]
            pub fn positive(x: u32) -> bool { x > 0 }
            "#,
        )
        .unwrap();

        let mut scan = AnnotationScan::default();
        scan_test_file(&mut scan, &temp.path().join("Cargo.toml"), &path).unwrap();

        assert_eq!(scan.function_specs.len(), 1);
        assert!(scan.function_specs[0].rust_theorem);
        assert_eq!(scan.function_specs[0].proof, Some(Proof::Auto));
        assert_eq!(scan.function_specs[0].spec.ensures.len(), 1);
        scan.reject_unsupported().unwrap();
    }

    #[test]
    fn scan_file_rejects_invalid_rust_theorem_shapes() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("lib.rs");
        std::fs::write(
            &path,
            r#"
            #[anneal(theorem)]
            fn private() -> bool { true }

            #[anneal(theorem)]
            pub fn wrong_return() -> u32 { 0 }

            #[anneal(theorem)]
            pub fn generic<T>() -> bool { true }

            #[anneal(theorem)]
            pub async fn asynchronous() -> bool { true }

            #[anneal(theorem)]
            pub fn mutable_input(x: &mut u32) -> bool { *x > 0 }

            #[anneal(theorem)]
            pub fn opaque_input(x: impl Copy) -> bool { let _ = x; true }
            "#,
        )
        .unwrap();

        let mut scan = AnnotationScan::default();
        scan_test_file(&mut scan, &temp.path().join("Cargo.toml"), &path).unwrap();

        let err = scan.reject_unsupported().unwrap_err().to_string();
        assert!(err.contains("unrestricted `pub` visibility"));
        assert!(err.contains("return exactly `bool`"));
        assert!(err.contains("cannot have generic parameters"));
        assert!(err.contains("cannot be async, const, extern, or variadic"));
        assert!(err.contains("cannot take or return mutable references"));
        assert!(err.contains("cannot use `impl Trait`"));
    }

    #[test]
    fn scan_file_rejects_unsupported_rust_condition_signatures_and_enclosing_generics() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("lib.rs");
        std::fs::write(
            &path,
            r#"
            #[anneal(spec(requires(prop = *x > 0)))]
            pub fn mutable_input(x: &mut u32) -> bool { *x > 0 }

            #[anneal(spec(ensures(prop = true)))]
            pub fn mutable_output() -> &'static mut u32 { unreachable!() }

            #[anneal(spec(requires(prop = true)))]
            pub fn opaque_input(x: impl Copy) -> bool { let _ = x; true }

            pub trait Generic<T> {
                #[anneal(spec(requires(prop = true)))]
                fn check(value: T) -> bool;
            }

            #[anneal(spec(invariant(prop = true)))]
            pub struct GenericStruct<T> {
                value: T,
            }
            "#,
        )
        .unwrap();

        let mut scan = AnnotationScan::default();
        scan_test_file(&mut scan, &temp.path().join("Cargo.toml"), &path).unwrap();

        let err = scan.reject_unsupported().unwrap_err().to_string();
        assert!(err.contains("mutable-reference inputs or outputs"));
        assert!(err.contains("using `impl Trait`"));
        assert!(err.contains("methods of generic traits"));
        assert!(err.contains("invariants on generic structs"));
    }

    #[test]
    fn scan_file_rejects_rust_theorem_on_trait_methods() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("lib.rs");
        std::fs::write(
            &path,
            r#"
            pub trait Lemmas {
                #[anneal(theorem)]
                fn lemma() -> bool;
            }
            "#,
        )
        .unwrap();

        let mut scan = AnnotationScan::default();
        scan_test_file(&mut scan, &temp.path().join("Cargo.toml"), &path).unwrap();

        let err = scan.reject_unsupported().unwrap_err().to_string();
        assert!(err.contains("supported only on public free functions"));
    }

    #[test]
    fn impl_auto_proof_overrides_inherited_trait_lean_proof() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("lib.rs");
        std::fs::write(
            &path,
            r#"
            pub trait Bounded {
                #[anneal(spec(requires = "x.val <= 10"), proof = "exact inherited")]
                fn bounded(x: u32) -> u32;
            }

            pub struct Impl;

            impl Bounded for Impl {
                #[anneal(proof(auto))]
                fn bounded(x: u32) -> u32 { x }
            }
            "#,
        )
        .unwrap();

        let mut scan = AnnotationScan::default();
        scan_test_file(&mut scan, &temp.path().join("Cargo.toml"), &path).unwrap();
        scan.expand_trait_function_specs();

        assert_eq!(scan.function_specs.len(), 1);
        assert_eq!(scan.function_specs[0].proof, Some(Proof::Auto));
        scan.reject_unsupported().unwrap();
    }

    #[test]
    fn scan_file_records_proof_only_free_functions_for_inferred_obligations() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("lib.rs");
        std::fs::write(
            &path,
            r#"
            #[anneal(proof = "simp_all")]
            pub fn f(x: u32) -> u32 { x }
            "#,
        )
        .unwrap();

        let mut scan = AnnotationScan::default();
        scan_test_file(&mut scan, &temp.path().join("Cargo.toml"), &path).unwrap();

        assert_eq!(scan.function_specs.len(), 1);
        assert!(scan.function_specs[0].spec.requires.is_empty());
        assert!(scan.function_specs[0].spec.ensures.is_empty());
        assert!(scan.function_specs[0].proof.is_some());
        scan.reject_unsupported().unwrap();
    }

    #[test]
    fn scan_file_records_auxiliary_theorem() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("lib.rs");
        std::fs::write(
            &path,
            r#"
            #[anneal(theorem(
                attr = "simp",
                name = "always_true",
                definition(ty = "True", proof = "trivial")
            ))]
            pub fn f(x: u32) -> u32 { x }
            "#,
        )
        .unwrap();

        let mut scan = AnnotationScan::default();
        scan_test_file(&mut scan, &temp.path().join("Cargo.toml"), &path).unwrap();

        assert_eq!(scan.aux_theorems.len(), 1);
        assert_eq!(scan.aux_theorems[0].theorem.name, "always_true");
        assert_eq!(scan.expansion_expectations.len(), 1);
        assert_eq!(
            scan.expansion_expectations[0].owner,
            ExpansionOwner::Named { target: ExpansionTarget::Function, path: vec!["f".into()] }
        );
        assert!(crate::annotations::is_expansion_marker(&scan.expansion_expectations[0].marker));
        scan.reject_unsupported().unwrap();
    }
}
