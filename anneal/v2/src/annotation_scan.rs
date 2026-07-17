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

use crate::annotations::{AnnotationTarget, AttributeData, Proof, Spec, Theorem};

/// Anneal attributes found while scanning Rust sources.
#[derive(Debug, Default)]
pub struct AnnotationScan {
    function_specs: Vec<FunctionSpec>,
    struct_invariants: Vec<StructInvariant>,
    aux_theorems: Vec<AuxTheorem>,
    trait_function_specs: Vec<TraitFunctionSpec>,
    impl_functions: Vec<ImplFunction>,
    unsupported: Vec<UnsupportedAnnotation>,
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
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FunctionMatchHint {
    Basename,
    TraitImpl { trait_name: String, self_ty: String, method_name: String },
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
    Trait { trait_name: String },
    Impl { trait_name: Option<String>, self_ty: Option<String> },
}

/// Scans Rust sources for the selected Anneal artifacts.
pub fn scan_packages(
    packages: &[crate::scanner::AnnealArtifact],
) -> anyhow::Result<AnnotationScan> {
    let mut scan = AnnotationScan::default();

    let mut visited = std::collections::BTreeSet::new();
    for package in packages {
        if !visited.insert(package.manifest_path.clone()) {
            continue;
        }
        let package_root = package.manifest_path.parent().ok_or_else(|| {
            anyhow::anyhow!(
                "package manifest {} has no parent directory",
                package.manifest_path.display()
            )
        })?;
        if let Some(src_path) = package_target_source(package) {
            scan_file(&mut scan, &package.manifest_path, package_root, src_path)?;
        } else {
            scan_package_root(&mut scan, &package.manifest_path, package_root)?;
        }
    }
    scan.expand_trait_function_specs();
    Ok(scan)
}

fn package_target_source(package: &crate::scanner::AnnealArtifact) -> Option<&std::path::Path> {
    match package.target_kind {
        crate::resolve::AnnealTargetKind::Bin
        | crate::resolve::AnnealTargetKind::Example
        | crate::resolve::AnnealTargetKind::Test => package.src_path.as_deref(),
        crate::resolve::AnnealTargetKind::Lib
        | crate::resolve::AnnealTargetKind::RLib
        | crate::resolve::AnnealTargetKind::ProcMacro
        | crate::resolve::AnnealTargetKind::CDyLib
        | crate::resolve::AnnealTargetKind::DyLib
        | crate::resolve::AnnealTargetKind::StaticLib => None,
    }
}

fn scan_package_root(
    scan: &mut AnnotationScan,
    package_manifest_path: &std::path::Path,
    package_root: &std::path::Path,
) -> anyhow::Result<()> {
    for entry in walkdir::WalkDir::new(package_root) {
        let entry = entry.with_context(|| format!("failed to walk {}", package_root.display()))?;
        let path = entry.path();
        if !entry.file_type().is_file()
            || path.extension().is_none_or(|extension| extension != "rs")
            || path.components().any(|component| component.as_os_str() == "target")
        {
            continue;
        }
        scan_file(scan, package_manifest_path, package_root, path)?;
    }
    Ok(())
}

fn scan_file(
    scan: &mut AnnotationScan,
    package_manifest_path: &std::path::Path,
    package_root: &std::path::Path,
    path: &std::path::Path,
) -> anyhow::Result<()> {
    let source = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read {}", path.display()))?;
    let file = syn::parse_file(&source)
        .with_context(|| format!("failed to parse Rust source {}", path.display()))?;
    let module_path = file_module_path(package_root, path);
    scan_items(scan, package_manifest_path, path, &module_path, &file.items)
}

fn scan_items(
    scan: &mut AnnotationScan,
    package_manifest_path: &std::path::Path,
    path: &std::path::Path,
    module_path: &[String],
    items: &[syn::Item],
) -> anyhow::Result<()> {
    for item in items {
        match item {
            syn::Item::Fn(item) => record_function_attrs(
                scan,
                package_manifest_path,
                path,
                AnnotationTarget::Function,
                FunctionContext::Free,
                item.sig.ident.to_string(),
                &item.attrs,
            )?,
            syn::Item::Struct(item) => {
                record_struct_attrs(scan, package_manifest_path, path, module_path, item)?;
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
                        record_function_attrs(
                            scan,
                            package_manifest_path,
                            path,
                            AnnotationTarget::TraitFunction,
                            FunctionContext::Trait { trait_name: item.ident.to_string() },
                            function.sig.ident.to_string(),
                            &function.attrs,
                        )?;
                    }
                }
            }
            syn::Item::Impl(item) => {
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
                        record_function_attrs(
                            scan,
                            package_manifest_path,
                            path,
                            AnnotationTarget::ImplFunction,
                            FunctionContext::Impl {
                                trait_name: trait_name.clone(),
                                self_ty: self_ty.clone(),
                            },
                            function.sig.ident.to_string(),
                            &function.attrs,
                        )?;
                    }
                }
            }
            syn::Item::Mod(item) => {
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
                    scan_items(scan, package_manifest_path, path, &child_module_path, items)?;
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

fn file_module_path(package_root: &std::path::Path, path: &std::path::Path) -> Vec<String> {
    let Ok(relative) = path.strip_prefix(package_root.join("src")) else {
        return Vec::new();
    };
    let mut components = relative
        .components()
        .filter_map(|component| component.as_os_str().to_str().map(str::to_string))
        .collect::<Vec<_>>();
    let Some(file_name) = components.pop() else {
        return Vec::new();
    };
    let stem = std::path::Path::new(&file_name)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or_default();
    match (components.is_empty(), stem) {
        (true, "lib" | "main") => Vec::new(),
        (_, "mod") => components,
        _ => {
            components.push(stem.to_string());
            components
        }
    }
}

fn item_attrs(item: &syn::Item) -> &[syn::Attribute] {
    match item {
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
            AttributeData::Spec(_) | AttributeData::Proof(_) => {
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
    path: &std::path::Path,
    module_path: &[String],
    item: &syn::ItemStruct,
) -> anyhow::Result<()> {
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
                    invariants: spec.invariants,
                });
            }
            AttributeData::Theorem(theorem) => {
                scan.aux_theorems.push(AuxTheorem {
                    package_manifest_path: package_manifest_path.to_path_buf(),
                    path: path.to_path_buf(),
                    theorem,
                });
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
    path: &std::path::Path,
    target: AnnotationTarget,
    context: FunctionContext,
    rust_name: String,
    attrs: &[syn::Attribute],
) -> anyhow::Result<()> {
    let attrs = crate::annotations::parse_attributes_for_target(attrs, target)
        .with_context(|| format!("failed to parse Anneal attributes in {}", path.display()))?;
    let mut spec = Spec::default();
    let mut proof = None;
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
            AttributeData::Theorem(theorem) => {
                scan.aux_theorems.push(AuxTheorem {
                    package_manifest_path: package_manifest_path.to_path_buf(),
                    path: path.to_path_buf(),
                    theorem,
                });
            }
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
                match_hint: FunctionMatchHint::Basename,
                spec,
                proof,
            });
        }
        FunctionContext::Trait { trait_name } => {
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
        AttributeData::Spec(_) | AttributeData::Theorem(_) | AttributeData::Proof(_) => {
            attr.domains.iter().any(|domain| domain == "soundness")
        }
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
        let package_root = path.parent().unwrap_or_else(|| std::path::Path::new("."));
        scan_file(scan, package_manifest_path, package_root, path)
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
            #[anneal(spec(requires = "x.val > 0", ensures = "True"))]
            pub fn f(x: u32) -> u32 { x }
            "#,
        )
        .unwrap();

        let mut scan = AnnotationScan::default();
        scan_test_file(&mut scan, &temp.path().join("Cargo.toml"), &path).unwrap();

        assert_eq!(scan.function_specs.len(), 1);
        assert_eq!(scan.function_specs[0].spec.requires.len(), 1);
        assert_eq!(scan.function_specs[0].spec.ensures.len(), 1);
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
        scan.reject_unsupported().unwrap();
    }
}
