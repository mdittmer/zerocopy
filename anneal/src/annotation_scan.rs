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

use crate::annotations::{AnnotationTarget, AttributeData};

/// Anneal attributes found while scanning Rust sources.
#[derive(Debug, Default)]
pub struct AnnotationScan {
    soundness_items: Vec<AnnotatedItem>,
}

impl AnnotationScan {
    /// Fails if soundness-domain annotations were found but cannot yet be interpreted.
    pub fn reject_unimplemented_soundness(&self) -> anyhow::Result<()> {
        if self.soundness_items.is_empty() {
            return Ok(());
        }

        let mut details = self
            .soundness_items
            .iter()
            .take(8)
            .map(|item| format!("{} ({:?})", item.path.display(), item.target))
            .collect::<Vec<_>>();
        if self.soundness_items.len() > details.len() {
            details.push(format!("... and {} more", self.soundness_items.len() - details.len()));
        }

        anyhow::bail!(
            "found {} Anneal soundness annotation item(s), but theorem generation is not implemented yet:\n{}",
            self.soundness_items.len(),
            details.join("\n")
        )
    }
}

#[derive(Debug)]
struct AnnotatedItem {
    path: std::path::PathBuf,
    target: AnnotationTarget,
}

/// Scans workspace-member Rust sources for Anneal attributes.
pub fn scan_workspace(metadata: &cargo_metadata::Metadata) -> anyhow::Result<AnnotationScan> {
    let mut scan = AnnotationScan::default();
    for package_id in &metadata.workspace_members {
        let Some(package) = metadata.packages.iter().find(|package| &package.id == package_id)
        else {
            continue;
        };
        let package_root = package.manifest_path.parent().ok_or_else(|| {
            anyhow::anyhow!("package {} manifest has no parent directory", package.name)
        })?;
        scan_package_root(&mut scan, package_root.as_std_path())?;
    }
    Ok(scan)
}

fn scan_package_root(
    scan: &mut AnnotationScan,
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
        scan_file(scan, path)?;
    }
    Ok(())
}

fn scan_file(scan: &mut AnnotationScan, path: &std::path::Path) -> anyhow::Result<()> {
    let source = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read {}", path.display()))?;
    let file = syn::parse_file(&source)
        .with_context(|| format!("failed to parse Rust source {}", path.display()))?;
    scan_items(scan, path, &file.items)
}

fn scan_items(
    scan: &mut AnnotationScan,
    path: &std::path::Path,
    items: &[syn::Item],
) -> anyhow::Result<()> {
    for item in items {
        match item {
            syn::Item::Fn(item) => {
                record_attrs(scan, path, AnnotationTarget::Function, &item.attrs)?
            }
            syn::Item::Struct(item) => {
                record_attrs(scan, path, AnnotationTarget::Struct, &item.attrs)?;
                for field in &item.fields {
                    record_attrs(scan, path, AnnotationTarget::Other, &field.attrs)?;
                }
            }
            syn::Item::Trait(item) => {
                record_attrs(scan, path, AnnotationTarget::Trait, &item.attrs)?;
                for trait_item in &item.items {
                    if let syn::TraitItem::Fn(function) = trait_item {
                        record_attrs(scan, path, AnnotationTarget::TraitFunction, &function.attrs)?;
                    }
                }
            }
            syn::Item::Impl(item) => {
                record_attrs(scan, path, AnnotationTarget::Impl, &item.attrs)?;
                for impl_item in &item.items {
                    if let syn::ImplItem::Fn(function) = impl_item {
                        record_attrs(scan, path, AnnotationTarget::ImplFunction, &function.attrs)?;
                    }
                }
            }
            syn::Item::Mod(item) => {
                record_attrs(scan, path, AnnotationTarget::Other, &item.attrs)?;
                if let Some((_, items)) = &item.content {
                    scan_items(scan, path, items)?;
                }
            }
            item => {
                record_attrs(scan, path, AnnotationTarget::Other, item_attrs(item))?;
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
    attrs: &[syn::Attribute],
) -> anyhow::Result<()> {
    let attrs = crate::annotations::parse_attributes_for_target(attrs, target)
        .with_context(|| format!("failed to parse Anneal attributes in {}", path.display()))?;
    if attrs.iter().any(is_soundness_attr) {
        scan.soundness_items.push(AnnotatedItem { path: path.to_path_buf(), target });
    }
    Ok(())
}

fn is_soundness_attr(attr: &crate::annotations::Attribute) -> bool {
    match &attr.data {
        AttributeData::Spec(_) | AttributeData::Proof(_) => {
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
        scan_file(&mut scan, &path).unwrap();

        assert_eq!(scan.soundness_items.len(), 1);
        assert!(scan.reject_unimplemented_soundness().is_err());
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
        scan_file(&mut scan, &path).unwrap();

        assert!(scan.soundness_items.is_empty());
        scan.reject_unimplemented_soundness().unwrap();
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
        let err = scan_file(&mut scan, &path).unwrap_err().to_string();

        assert!(err.contains("failed to parse Anneal attributes"));
    }
}
