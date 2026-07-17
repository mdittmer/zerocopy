// Copyright 2026 The Fuchsia Authors
//
// Licensed under the 2-Clause BSD License <LICENSE-BSD or
// https://opensource.org/license/bsd-2-clause>, Apache License, Version 2.0
// <LICENSE-APACHE or https://www.apache.org/licenses/LICENSE-2.0>, or the MIT
// license <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to
// those terms.

//! A Rust source view suitable for rustc and Charon.
//!
//! Anneal annotations are first-class in user source, but rustc rejects unknown
//! outer attributes. The pipeline scans the original source first, then runs
//! Charon against this derived view with `#[anneal(...)]` attributes removed.

use anyhow::Context as _;
use quote::ToTokens as _;

pub struct SourceView {
    manifest_paths: std::collections::BTreeMap<std::path::PathBuf, std::path::PathBuf>,
}

impl SourceView {
    pub fn prepare(
        roots: &crate::resolve::LockedRoots,
        packages: &[crate::scanner::AnnealArtifact],
    ) -> anyhow::Result<Self> {
        let workspace_root = roots.workspace_root();
        if !workspace_contains_first_class_anneal_attrs(workspace_root)? {
            return Ok(Self { manifest_paths: std::collections::BTreeMap::new() });
        }

        let source_root = roots.source_root();
        if source_root.exists() {
            std::fs::remove_dir_all(&source_root)
                .with_context(|| format!("failed to clear {}", source_root.display()))?;
        }
        copy_workspace_stripping_attrs(workspace_root, &source_root)?;

        let mut manifest_paths = std::collections::BTreeMap::new();
        for artifact in packages {
            if let Ok(relative) = artifact.manifest_path.strip_prefix(workspace_root) {
                manifest_paths.insert(artifact.manifest_path.clone(), source_root.join(relative));
            }
        }

        Ok(Self { manifest_paths })
    }

    pub fn manifest_path<'a>(
        &'a self,
        artifact: &'a crate::scanner::AnnealArtifact,
    ) -> std::borrow::Cow<'a, std::path::Path> {
        self.manifest_paths
            .get(&artifact.manifest_path)
            .map(|path| std::borrow::Cow::Borrowed(path.as_path()))
            .unwrap_or_else(|| std::borrow::Cow::Borrowed(artifact.manifest_path.as_path()))
    }
}

fn workspace_contains_first_class_anneal_attrs(
    workspace_root: &std::path::Path,
) -> anyhow::Result<bool> {
    for entry in walkdir::WalkDir::new(workspace_root)
        .into_iter()
        .filter_entry(|entry| !is_skipped_dir(entry))
    {
        let entry =
            entry.with_context(|| format!("failed to walk {}", workspace_root.display()))?;
        if !entry.file_type().is_file() || entry.path().extension().is_none_or(|ext| ext != "rs") {
            continue;
        }
        let source = std::fs::read_to_string(entry.path())
            .with_context(|| format!("failed to read {}", entry.path().display()))?;
        if source.contains("#[anneal") || source.contains("#![anneal") {
            return Ok(true);
        }
    }
    Ok(false)
}

fn copy_workspace_stripping_attrs(
    workspace_root: &std::path::Path,
    source_root: &std::path::Path,
) -> anyhow::Result<()> {
    for entry in walkdir::WalkDir::new(workspace_root)
        .into_iter()
        .filter_entry(|entry| !is_skipped_dir(entry))
    {
        let entry =
            entry.with_context(|| format!("failed to walk {}", workspace_root.display()))?;
        let relative = entry.path().strip_prefix(workspace_root).with_context(|| {
            format!(
                "failed to compute relative path from {} to {}",
                workspace_root.display(),
                entry.path().display()
            )
        })?;
        let dest = source_root.join(relative);

        if entry.file_type().is_dir() {
            std::fs::create_dir_all(&dest)
                .with_context(|| format!("failed to create {}", dest.display()))?;
        } else if entry.file_type().is_file() {
            if let Some(parent) = dest.parent() {
                std::fs::create_dir_all(parent)
                    .with_context(|| format!("failed to create {}", parent.display()))?;
            }
            if entry.path().extension().is_some_and(|ext| ext == "rs") {
                let source = std::fs::read_to_string(entry.path())
                    .with_context(|| format!("failed to read {}", entry.path().display()))?;
                let stripped = strip_anneal_attrs(&source).with_context(|| {
                    format!("failed to strip Anneal attributes from {}", entry.path().display())
                })?;
                std::fs::write(&dest, stripped)
                    .with_context(|| format!("failed to write {}", dest.display()))?;
            } else {
                std::fs::copy(entry.path(), &dest).with_context(|| {
                    format!("failed to copy {} to {}", entry.path().display(), dest.display())
                })?;
            }
        }
    }
    Ok(())
}

fn is_skipped_dir(entry: &walkdir::DirEntry) -> bool {
    if entry.depth() == 0 || !entry.file_type().is_dir() {
        return false;
    }
    matches!(entry.file_name().to_str(), Some(".git" | "target"))
}

fn strip_anneal_attrs(source: &str) -> anyhow::Result<String> {
    if !(source.contains("#[anneal") || source.contains("#![anneal")) {
        return Ok(source.to_owned());
    }

    let mut file = syn::parse_file(source)?;
    strip_attrs(&mut file.attrs);
    for item in &mut file.items {
        strip_item(item);
    }
    let mut out = file.into_token_stream().to_string();
    out.push('\n');
    Ok(out)
}

fn strip_item(item: &mut syn::Item) {
    match item {
        syn::Item::Const(item) => strip_attrs(&mut item.attrs),
        syn::Item::Enum(item) => {
            strip_attrs(&mut item.attrs);
            for variant in &mut item.variants {
                strip_attrs(&mut variant.attrs);
                strip_fields(&mut variant.fields);
            }
        }
        syn::Item::ExternCrate(item) => strip_attrs(&mut item.attrs),
        syn::Item::Fn(item) => strip_attrs(&mut item.attrs),
        syn::Item::ForeignMod(item) => {
            strip_attrs(&mut item.attrs);
            for item in &mut item.items {
                strip_foreign_item(item);
            }
        }
        syn::Item::Impl(item) => {
            strip_attrs(&mut item.attrs);
            for item in &mut item.items {
                strip_impl_item(item);
            }
        }
        syn::Item::Macro(item) => strip_attrs(&mut item.attrs),
        syn::Item::Mod(item) => {
            strip_attrs(&mut item.attrs);
            if let Some((_, items)) = &mut item.content {
                for item in items {
                    strip_item(item);
                }
            }
        }
        syn::Item::Static(item) => strip_attrs(&mut item.attrs),
        syn::Item::Struct(item) => {
            strip_attrs(&mut item.attrs);
            strip_fields(&mut item.fields);
        }
        syn::Item::Trait(item) => {
            strip_attrs(&mut item.attrs);
            for item in &mut item.items {
                strip_trait_item(item);
            }
        }
        syn::Item::TraitAlias(item) => strip_attrs(&mut item.attrs),
        syn::Item::Type(item) => strip_attrs(&mut item.attrs),
        syn::Item::Union(item) => {
            strip_attrs(&mut item.attrs);
            strip_fields_named(&mut item.fields);
        }
        syn::Item::Use(item) => strip_attrs(&mut item.attrs),
        _ => {}
    }
}

fn strip_fields(fields: &mut syn::Fields) {
    for field in fields {
        strip_attrs(&mut field.attrs);
    }
}

fn strip_fields_named(fields: &mut syn::FieldsNamed) {
    for field in &mut fields.named {
        strip_attrs(&mut field.attrs);
    }
}

fn strip_trait_item(item: &mut syn::TraitItem) {
    match item {
        syn::TraitItem::Const(item) => strip_attrs(&mut item.attrs),
        syn::TraitItem::Fn(item) => strip_attrs(&mut item.attrs),
        syn::TraitItem::Macro(item) => strip_attrs(&mut item.attrs),
        syn::TraitItem::Type(item) => strip_attrs(&mut item.attrs),
        _ => {}
    }
}

fn strip_impl_item(item: &mut syn::ImplItem) {
    match item {
        syn::ImplItem::Const(item) => strip_attrs(&mut item.attrs),
        syn::ImplItem::Fn(item) => strip_attrs(&mut item.attrs),
        syn::ImplItem::Macro(item) => strip_attrs(&mut item.attrs),
        syn::ImplItem::Type(item) => strip_attrs(&mut item.attrs),
        _ => {}
    }
}

fn strip_foreign_item(item: &mut syn::ForeignItem) {
    match item {
        syn::ForeignItem::Fn(item) => strip_attrs(&mut item.attrs),
        syn::ForeignItem::Macro(item) => strip_attrs(&mut item.attrs),
        syn::ForeignItem::Static(item) => strip_attrs(&mut item.attrs),
        syn::ForeignItem::Type(item) => strip_attrs(&mut item.attrs),
        _ => {}
    }
}

fn strip_attrs(attrs: &mut Vec<syn::Attribute>) {
    attrs.retain(|attr| !is_anneal_attr(attr));
}

fn is_anneal_attr(attr: &syn::Attribute) -> bool {
    attr.path().segments.first().is_some_and(|segment| segment.ident == "anneal")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strip_anneal_attrs_removes_item_and_field_annotations() {
        let stripped = strip_anneal_attrs(
            r#"
            #[derive(Clone)]
            #[anneal(spec(invariant = "self.value.val <= 10"))]
            pub struct Counter {
                #[anneal(proof = "sorry")]
                value: u32,
            }

            #[anneal(spec(requires = "value.val <= 10"))]
            pub fn make(value: u32) -> Counter {
                Counter { value }
            }
            "#,
        )
        .unwrap();

        assert!(!stripped.contains("anneal"));
        assert!(stripped.contains("derive"));
        assert!(stripped.contains("pub struct Counter"));
        assert!(stripped.contains("pub fn make"));
    }

    #[test]
    fn strip_anneal_attrs_leaves_unannotated_source_unchanged() {
        let source = "pub fn f(x: u32) -> u32 { x }\n";

        assert_eq!(strip_anneal_attrs(source).unwrap(), source);
    }
}
