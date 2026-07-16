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

use crate::annotations::{AnnotationTarget, AttributeData, Proof, Spec};

/// Anneal attributes found while scanning Rust sources.
#[derive(Debug, Default)]
pub struct AnnotationScan {
    function_specs: Vec<FunctionSpec>,
    unsupported: Vec<UnsupportedAnnotation>,
}

impl AnnotationScan {
    /// Function-level postcondition specs that the current generator supports.
    pub fn function_specs(&self) -> &[FunctionSpec] {
        &self.function_specs
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
}

#[derive(Debug)]
pub struct FunctionSpec {
    pub package_manifest_path: std::path::PathBuf,
    pub path: std::path::PathBuf,
    pub rust_name: String,
    pub spec: Spec,
    pub proof: Option<Proof>,
}

#[derive(Debug)]
struct UnsupportedAnnotation {
    path: std::path::PathBuf,
    rust_name: Option<String>,
    target: AnnotationTarget,
    reason: String,
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
        scan_package_root(&mut scan, &package.manifest_path, package_root)?;
    }
    Ok(scan)
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
        scan_file(scan, package_manifest_path, path)?;
    }
    Ok(())
}

fn scan_file(
    scan: &mut AnnotationScan,
    package_manifest_path: &std::path::Path,
    path: &std::path::Path,
) -> anyhow::Result<()> {
    let source = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read {}", path.display()))?;
    let file = syn::parse_file(&source)
        .with_context(|| format!("failed to parse Rust source {}", path.display()))?;
    scan_items(scan, package_manifest_path, path, &file.items)
}

fn scan_items(
    scan: &mut AnnotationScan,
    package_manifest_path: &std::path::Path,
    path: &std::path::Path,
    items: &[syn::Item],
) -> anyhow::Result<()> {
    for item in items {
        match item {
            syn::Item::Fn(item) => record_function_attrs(
                scan,
                package_manifest_path,
                path,
                AnnotationTarget::Function,
                item.sig.ident.to_string(),
                &item.attrs,
            )?,
            syn::Item::Struct(item) => {
                record_attrs(scan, path, AnnotationTarget::Struct, None, &item.attrs)?;
                for field in &item.fields {
                    record_attrs(scan, path, AnnotationTarget::Other, None, &field.attrs)?;
                }
            }
            syn::Item::Trait(item) => {
                record_attrs(
                    scan,
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
                            function.sig.ident.to_string(),
                            &function.attrs,
                        )?;
                    }
                }
            }
            syn::Item::Impl(item) => {
                record_attrs(scan, path, AnnotationTarget::Impl, None, &item.attrs)?;
                for impl_item in &item.items {
                    if let syn::ImplItem::Fn(function) = impl_item {
                        record_function_attrs(
                            scan,
                            package_manifest_path,
                            path,
                            AnnotationTarget::ImplFunction,
                            function.sig.ident.to_string(),
                            &function.attrs,
                        )?;
                    }
                }
            }
            syn::Item::Mod(item) => {
                record_attrs(
                    scan,
                    path,
                    AnnotationTarget::Other,
                    Some(item.ident.to_string()),
                    &item.attrs,
                )?;
                if let Some((_, items)) = &item.content {
                    scan_items(scan, package_manifest_path, path, items)?;
                }
            }
            item => {
                record_attrs(scan, path, AnnotationTarget::Other, None, item_attrs(item))?;
            }
        }
    }
    Ok(())
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

fn record_attrs(
    scan: &mut AnnotationScan,
    path: &std::path::Path,
    target: AnnotationTarget,
    rust_name: Option<String>,
    attrs: &[syn::Attribute],
) -> anyhow::Result<()> {
    let attrs = crate::annotations::parse_attributes_for_target(attrs, target)
        .with_context(|| format!("failed to parse Anneal attributes in {}", path.display()))?;
    if attrs.iter().any(is_soundness_attr) {
        scan.unsupported.push(UnsupportedAnnotation {
            path: path.to_path_buf(),
            rust_name,
            target,
            reason: format!("soundness annotations on {target:?} are not implemented yet"),
        });
    }
    Ok(())
}

fn record_function_attrs(
    scan: &mut AnnotationScan,
    package_manifest_path: &std::path::Path,
    path: &std::path::Path,
    target: AnnotationTarget,
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
            AttributeData::Theorem(_) => {
                push_unsupported(
                    scan,
                    path,
                    target,
                    Some(rust_name.clone()),
                    "theorem annotations are parsed but not integrated into soundness generation yet",
                );
                supported = false;
            }
        }
    }

    if !has_soundness {
        return Ok(());
    }

    if target != AnnotationTarget::Function {
        push_unsupported(
            scan,
            path,
            target,
            Some(rust_name),
            "soundness annotations on trait or impl functions are not implemented yet",
        );
        return Ok(());
    }

    if !spec.requires.is_empty() {
        push_unsupported(
            scan,
            path,
            target,
            Some(rust_name.clone()),
            "preconditions are parsed but not integrated into generated soundness theorems yet",
        );
        supported = false;
    }
    if !spec.invariants.is_empty() {
        push_unsupported(
            scan,
            path,
            target,
            Some(rust_name.clone()),
            "invariants are parsed but not integrated into generated soundness theorems yet",
        );
        supported = false;
    }
    if spec.ensures.is_empty() {
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

    if supported {
        scan.function_specs.push(FunctionSpec {
            package_manifest_path: package_manifest_path.to_path_buf(),
            path: path.to_path_buf(),
            rust_name,
            spec,
            proof,
        });
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
        scan_file(&mut scan, &temp.path().join("Cargo.toml"), &path).unwrap();

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
        scan_file(&mut scan, &temp.path().join("Cargo.toml"), &path).unwrap();

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
        let err =
            scan_file(&mut scan, &temp.path().join("Cargo.toml"), &path).unwrap_err().to_string();

        assert!(err.contains("failed to parse Anneal attributes"));
    }

    #[test]
    fn scan_file_rejects_preconditions_until_supported() {
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
        scan_file(&mut scan, &temp.path().join("Cargo.toml"), &path).unwrap();

        assert!(scan.function_specs.is_empty());
        let err = scan.reject_unsupported().unwrap_err().to_string();
        assert!(err.contains("preconditions are parsed"));
    }

    #[test]
    fn scan_file_rejects_trait_function_specs_until_supported() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("lib.rs");
        std::fs::write(
            &path,
            r#"
            pub trait Demo {
                #[anneal(spec(ensures = "True"))]
                fn f(&self) -> u32;
            }
            "#,
        )
        .unwrap();

        let mut scan = AnnotationScan::default();
        scan_file(&mut scan, &temp.path().join("Cargo.toml"), &path).unwrap();

        assert!(scan.function_specs.is_empty());
        let err = scan.reject_unsupported().unwrap_err().to_string();
        assert!(err.contains("trait or impl functions are not implemented"));
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
        scan_file(&mut scan, &temp.path().join("Cargo.toml"), &path).unwrap();

        assert_eq!(scan.function_specs.len(), 1);
        assert!(scan.function_specs[0].proof.is_some());
        scan.reject_unsupported().unwrap();
    }
}
