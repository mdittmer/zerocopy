// Copyright 2026 The Fuchsia Authors
//
// Licensed under the 2-Clause BSD License <LICENSE-BSD or
// https://opensource.org/license/bsd-2-clause>, Apache License, Version 2.0
// <LICENSE-APACHE or https://www.apache.org/licenses/LICENSE-2.0>, or the MIT
// license <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to
// those terms.

//! Extraction of Anneal v2 annotations from Charon metadata.
//!
//! The user-facing syntax is real Rust item attributes from the
//! `anneal-annotations` proc-macro crate. Those macros lower to hidden doc
//! metadata because Charon preserves doc comments in LLBC. This module treats
//! the LLBC metadata as the source of truth; it never scans Rust source files.

use anyhow::Context as _;
use charon_lib::ast::{AttrInfo, Attribute, Body, ItemMeta, ItemOpacity, Name, PathElem};

pub const METADATA_PREFIX: &str = "anneal:v2:";

const SUPPORTED_METADATA_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct Annotation {
    pub version: u32,
    pub kind: AnnotationKind,
    pub sections: Vec<AnnotationSection>,
    #[serde(default)]
    pub argument_names: Option<Vec<String>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AnnotationKind {
    Spec,
    UnsafeAxiom,
    TypeInvariant,
    TraitInvariant,
    ImplInvariant,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct AnnotationSection {
    pub kind: SectionKind,
    pub name: Option<String>,
    pub lean: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SectionKind {
    Context,
    Requires,
    Ensures,
    ProofContext,
    Proof,
    IsValid,
    IsSafe,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AnnotationBody {
    Function(FunctionAnnotation),
    TypeInvariant(TypeInvariantAnnotation),
    TraitInvariant(TraitInvariantAnnotation),
    ImplInvariant(ImplInvariantAnnotation),
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct FunctionAnnotation {
    pub kind: FunctionAnnotationKind,
    pub context: Vec<LeanFragment>,
    pub requires: Vec<LeanFragment>,
    pub ensures: Vec<LeanFragment>,
    pub proof_context: Vec<LeanFragment>,
    pub proofs: Vec<LeanFragment>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FunctionAnnotationKind {
    Spec,
    UnsafeAxiom,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct TypeInvariantAnnotation {
    pub context: Vec<LeanFragment>,
    pub is_valid: Vec<LeanFragment>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct TraitInvariantAnnotation {
    pub context: Vec<LeanFragment>,
    pub is_safe: Vec<LeanFragment>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ImplInvariantAnnotation {
    pub context: Vec<LeanFragment>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct LeanFragment {
    pub name: Option<String>,
    pub lean: String,
}

#[derive(Debug, Clone)]
pub struct AnnotatedArtifact {
    pub artifact: crate::scanner::AnnealArtifact,
    pub items: Vec<AnnotatedItem>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct AnnotatedItem {
    pub item_id: AnnotatedItemId,
    pub item_kind: AnnotatedItemKind,
    pub path: Vec<String>,
    pub annotation: Annotation,
    pub body: AnnotationBody,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct AnnotatedItemId {
    pub kind: AnnotatedItemKind,
    pub index: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AnnotatedItemKind {
    Function,
    Type,
    Trait,
    Impl,
    Global,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("failed to parse Anneal metadata on `{path}`: {source}")]
    InvalidMetadata { path: String, source: serde_json::Error },
    #[error("unsupported Anneal metadata version {version} on `{path}`")]
    UnsupportedVersion { path: String, version: u32 },
    #[error("multiple Anneal annotations found on `{path}`")]
    MultipleAnnotations { path: String },
    #[error("`#[anneal::{annotation}]` is not valid on {item_kind} `{path}`")]
    InvalidItemKind { path: String, item_kind: &'static str, annotation: &'static str },
    #[error("`{section}` sections are not valid in `#[anneal::{annotation}]` metadata on `{path}`")]
    InvalidSection { path: String, annotation: &'static str, section: &'static str },
    #[error("`#[anneal::unsafe_axiom]` did not make `{path}` opaque in LLBC")]
    UnsafeAxiomNotOpaque { path: String },
    #[error("`#[anneal::spec]` cannot verify opaque function `{path}`")]
    SpecIsOpaque { path: String },
    #[error("`requires` sections are only permitted on unsafe functions; `{path}` is safe")]
    RequiresSafeFunction { path: String },
    #[error("`#[anneal::{annotation}]` on `{path}` is missing a `{section}` section")]
    MissingRequiredSection { path: String, annotation: &'static str, section: &'static str },
    #[error("`#[anneal::impl_invariant]` on `{path}` is parsed but not supported yet")]
    ImplInvariantUnsupported { path: String },
}

pub fn extract_crate_annotations(
    crate_data: &charon_lib::export::CrateData,
) -> Result<Vec<AnnotatedItem>, Error> {
    let krate = &crate_data.translated;
    let mut items = Vec::new();

    for fun in &krate.fun_decls {
        let facts = ItemFacts {
            kind: AnnotatedItemKind::Function,
            opacity: ItemOpacity::Transparent,
            is_unsafe_function: Some(fun.signature.is_unsafe),
            is_opaque_function: Some(matches!(fun.body, Body::Opaque)),
        };
        maybe_push_annotation(
            &mut items,
            AnnotatedItemId::new(AnnotatedItemKind::Function, fun.def_id.index()),
            &fun.item_meta,
            facts,
        )?;
    }
    for ty in &krate.type_decls {
        maybe_push_annotation(
            &mut items,
            AnnotatedItemId::new(AnnotatedItemKind::Type, ty.def_id.index()),
            &ty.item_meta,
            ItemFacts::plain(AnnotatedItemKind::Type),
        )?;
    }
    for tr in &krate.trait_decls {
        maybe_push_annotation(
            &mut items,
            AnnotatedItemId::new(AnnotatedItemKind::Trait, tr.def_id.index()),
            &tr.item_meta,
            ItemFacts::plain(AnnotatedItemKind::Trait),
        )?;
    }
    for imp in &krate.trait_impls {
        maybe_push_annotation(
            &mut items,
            AnnotatedItemId::new(AnnotatedItemKind::Impl, imp.def_id.index()),
            &imp.item_meta,
            ItemFacts::plain(AnnotatedItemKind::Impl),
        )?;
    }
    for global in &krate.global_decls {
        maybe_push_annotation(
            &mut items,
            AnnotatedItemId::new(AnnotatedItemKind::Global, global.def_id.index()),
            &global.item_meta,
            ItemFacts::plain(AnnotatedItemKind::Global),
        )?;
    }

    Ok(items)
}

pub fn extract_artifact_annotations(
    roots: &crate::resolve::LockedRoots<'_>,
    artifacts: &[crate::scanner::AnnealArtifact],
) -> anyhow::Result<Vec<AnnotatedArtifact>> {
    artifacts
        .iter()
        .map(|artifact| {
            let llbc_path = artifact.llbc_path(roots);
            let file = std::fs::File::open(&llbc_path).map_err(anyhow::Error::from).with_context(
                || format!("failed to open LLBC artifact `{}`", llbc_path.display()),
            )?;
            let crate_data: charon_lib::export::CrateData = serde_json::from_reader(file)
                .with_context(|| {
                    format!("failed to parse LLBC artifact `{}`", llbc_path.display())
                })?;
            let items = extract_crate_annotations(&crate_data).with_context(|| {
                format!("failed to extract Anneal annotations from `{}`", llbc_path.display())
            })?;
            Ok(AnnotatedArtifact { artifact: artifact.clone(), items })
        })
        .collect()
}

pub fn write_annotation_manifests(
    roots: &crate::resolve::LockedRoots<'_>,
    artifacts: &[AnnotatedArtifact],
) -> anyhow::Result<Vec<std::path::PathBuf>> {
    let manifest_root = roots.llbc_root().join("annotations");
    std::fs::create_dir_all(&manifest_root)
        .with_context(|| format!("failed to create `{}`", manifest_root.display()))?;

    artifacts
        .iter()
        .map(|artifact| {
            let path = manifest_root.join(format!("{}.annotations.json", artifact.artifact_slug()));
            let mut json = serde_json::to_string_pretty(&annotation_manifest(artifact))
                .context("failed to serialize Anneal annotation manifest")?;
            json.push('\n');
            std::fs::write(&path, json)
                .with_context(|| format!("failed to write `{}`", path.display()))?;
            Ok(path)
        })
        .collect()
}

#[derive(Debug, serde::Serialize)]
struct AnnotationManifest<'a> {
    artifact: AnnotationManifestArtifact,
    items: &'a [AnnotatedItem],
}

#[derive(Debug, serde::Serialize)]
struct AnnotationManifestArtifact {
    package_name: String,
    target_name: String,
    target_kind: String,
    llbc_file: String,
}

fn annotation_manifest(artifact: &AnnotatedArtifact) -> AnnotationManifest<'_> {
    AnnotationManifest {
        artifact: AnnotationManifestArtifact {
            package_name: artifact.artifact.name.package_name.to_string(),
            target_name: artifact.artifact.name.target_name.clone(),
            target_kind: format!("{:?}", artifact.artifact.target_kind),
            llbc_file: artifact.artifact.llbc_file_name(),
        },
        items: &artifact.items,
    }
}

impl AnnotatedArtifact {
    fn artifact_slug(&self) -> String {
        self.artifact.artifact_slug()
    }
}

fn maybe_push_annotation(
    items: &mut Vec<AnnotatedItem>,
    item_id: AnnotatedItemId,
    meta: &ItemMeta,
    facts: ItemFacts,
) -> Result<(), Error> {
    let path = path_segments(&meta.name);
    let Some(annotation) = extract_annotation(&meta.attr_info, &path)? else {
        return Ok(());
    };
    let body = validate_annotation(&annotation, facts.with_opacity(meta.opacity), &path)?;
    items.push(AnnotatedItem { item_id, item_kind: facts.kind, path, annotation, body });
    Ok(())
}

fn extract_annotation(attr_info: &AttrInfo, path: &[String]) -> Result<Option<Annotation>, Error> {
    let path = display_path(path);
    let mut annotation = None;

    for attr in &attr_info.attributes {
        let Attribute::DocComment(comment) = attr else {
            continue;
        };
        let Some(payload) = comment.trim_start().strip_prefix(METADATA_PREFIX) else {
            continue;
        };
        if annotation.is_some() {
            return Err(Error::MultipleAnnotations { path });
        }
        let parsed: Annotation = serde_json::from_str(payload)
            .map_err(|source| Error::InvalidMetadata { path: path.clone(), source })?;
        if parsed.version != SUPPORTED_METADATA_VERSION {
            return Err(Error::UnsupportedVersion { path: path.clone(), version: parsed.version });
        }
        annotation = Some(parsed);
    }

    Ok(annotation)
}

#[derive(Debug, Clone, Copy)]
struct ItemFacts {
    kind: AnnotatedItemKind,
    opacity: ItemOpacity,
    is_unsafe_function: Option<bool>,
    is_opaque_function: Option<bool>,
}

impl ItemFacts {
    fn plain(kind: AnnotatedItemKind) -> Self {
        Self {
            kind,
            opacity: ItemOpacity::Transparent,
            is_unsafe_function: None,
            is_opaque_function: None,
        }
    }

    fn with_opacity(mut self, opacity: ItemOpacity) -> Self {
        self.opacity = opacity;
        self
    }
}

fn validate_annotation(
    annotation: &Annotation,
    facts: ItemFacts,
    path: &[String],
) -> Result<AnnotationBody, Error> {
    use AnnotatedItemKind::*;

    let path = display_path(path);
    let body = annotation_body(annotation, &path)?;
    match (&body, facts.kind) {
        (AnnotationBody::Function(function), Function) => {
            match function.kind {
                FunctionAnnotationKind::Spec => {
                    if facts.is_opaque_function == Some(true)
                        || facts.opacity == ItemOpacity::Opaque
                    {
                        return Err(Error::SpecIsOpaque { path });
                    }
                }
                FunctionAnnotationKind::UnsafeAxiom => {
                    if facts.is_opaque_function != Some(true)
                        && facts.opacity != ItemOpacity::Opaque
                    {
                        return Err(Error::UnsafeAxiomNotOpaque { path });
                    }
                }
            }
            if !function.requires.is_empty() && facts.is_unsafe_function == Some(false) {
                return Err(Error::RequiresSafeFunction { path });
            }
        }
        (AnnotationBody::TypeInvariant(ty), Type) => {
            require_fragments(&ty.is_valid, &path, "type_invariant", "is_valid")?;
        }
        (AnnotationBody::TraitInvariant(tr), Trait) => {
            require_fragments(&tr.is_safe, &path, "trait_invariant", "is_safe")?;
        }
        (AnnotationBody::ImplInvariant(_), Impl) => {
            return Err(Error::ImplInvariantUnsupported { path });
        }
        (body, item_kind) => {
            return Err(Error::InvalidItemKind {
                path,
                item_kind: item_kind.as_str(),
                annotation: body.annotation_kind_str(),
            });
        }
    }

    Ok(body)
}

fn annotation_body(annotation: &Annotation, path: &str) -> Result<AnnotationBody, Error> {
    let invalid = |section: SectionKind| Error::InvalidSection {
        path: path.to_string(),
        annotation: annotation.kind.as_str(),
        section: section.as_str(),
    };

    match annotation.kind {
        AnnotationKind::Spec => {
            let mut body = FunctionAnnotation {
                kind: FunctionAnnotationKind::Spec,
                context: Vec::new(),
                requires: Vec::new(),
                ensures: Vec::new(),
                proof_context: Vec::new(),
                proofs: Vec::new(),
            };
            for section in &annotation.sections {
                match section.kind {
                    SectionKind::Context => body.context.push(section.fragment()),
                    SectionKind::Requires => body.requires.push(section.fragment()),
                    SectionKind::Ensures => body.ensures.push(section.fragment()),
                    SectionKind::ProofContext => body.proof_context.push(section.fragment()),
                    SectionKind::Proof => body.proofs.push(section.fragment()),
                    kind @ (SectionKind::IsValid | SectionKind::IsSafe) => {
                        return Err(invalid(kind));
                    }
                }
            }
            Ok(AnnotationBody::Function(body))
        }
        AnnotationKind::UnsafeAxiom => {
            let mut body = FunctionAnnotation {
                kind: FunctionAnnotationKind::UnsafeAxiom,
                context: Vec::new(),
                requires: Vec::new(),
                ensures: Vec::new(),
                proof_context: Vec::new(),
                proofs: Vec::new(),
            };
            for section in &annotation.sections {
                match section.kind {
                    SectionKind::Context => body.context.push(section.fragment()),
                    SectionKind::Requires => body.requires.push(section.fragment()),
                    SectionKind::Ensures => body.ensures.push(section.fragment()),
                    kind @ (SectionKind::ProofContext
                    | SectionKind::Proof
                    | SectionKind::IsValid
                    | SectionKind::IsSafe) => return Err(invalid(kind)),
                }
            }
            Ok(AnnotationBody::Function(body))
        }
        AnnotationKind::TypeInvariant => {
            let mut body = TypeInvariantAnnotation { context: Vec::new(), is_valid: Vec::new() };
            for section in &annotation.sections {
                match section.kind {
                    SectionKind::Context => body.context.push(section.fragment()),
                    SectionKind::IsValid => body.is_valid.push(section.fragment()),
                    kind @ (SectionKind::Requires
                    | SectionKind::Ensures
                    | SectionKind::ProofContext
                    | SectionKind::Proof
                    | SectionKind::IsSafe) => return Err(invalid(kind)),
                }
            }
            Ok(AnnotationBody::TypeInvariant(body))
        }
        AnnotationKind::TraitInvariant => {
            let mut body = TraitInvariantAnnotation { context: Vec::new(), is_safe: Vec::new() };
            for section in &annotation.sections {
                match section.kind {
                    SectionKind::Context => body.context.push(section.fragment()),
                    SectionKind::IsSafe => body.is_safe.push(section.fragment()),
                    kind @ (SectionKind::Requires
                    | SectionKind::Ensures
                    | SectionKind::ProofContext
                    | SectionKind::Proof
                    | SectionKind::IsValid) => return Err(invalid(kind)),
                }
            }
            Ok(AnnotationBody::TraitInvariant(body))
        }
        AnnotationKind::ImplInvariant => {
            let mut body = ImplInvariantAnnotation { context: Vec::new() };
            for section in &annotation.sections {
                match section.kind {
                    SectionKind::Context => body.context.push(section.fragment()),
                    kind @ (SectionKind::Requires
                    | SectionKind::Ensures
                    | SectionKind::ProofContext
                    | SectionKind::Proof
                    | SectionKind::IsValid
                    | SectionKind::IsSafe) => return Err(invalid(kind)),
                }
            }
            Ok(AnnotationBody::ImplInvariant(body))
        }
    }
}

fn require_fragments(
    fragments: &[LeanFragment],
    path: &str,
    annotation_name: &'static str,
    section_name: &'static str,
) -> Result<(), Error> {
    if !fragments.is_empty() {
        Ok(())
    } else {
        Err(Error::MissingRequiredSection {
            path: path.to_string(),
            annotation: annotation_name,
            section: section_name,
        })
    }
}

impl AnnotationBody {
    fn annotation_kind_str(&self) -> &'static str {
        match self {
            Self::Function(function) => match function.kind {
                FunctionAnnotationKind::Spec => "spec",
                FunctionAnnotationKind::UnsafeAxiom => "unsafe_axiom",
            },
            Self::TypeInvariant(_) => "type_invariant",
            Self::TraitInvariant(_) => "trait_invariant",
            Self::ImplInvariant(_) => "impl_invariant",
        }
    }
}

impl AnnotationKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Spec => "spec",
            Self::UnsafeAxiom => "unsafe_axiom",
            Self::TypeInvariant => "type_invariant",
            Self::TraitInvariant => "trait_invariant",
            Self::ImplInvariant => "impl_invariant",
        }
    }
}

impl SectionKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Context => "context",
            Self::Requires => "requires",
            Self::Ensures => "ensures",
            Self::ProofContext => "proof_context",
            Self::Proof => "proof",
            Self::IsValid => "is_valid",
            Self::IsSafe => "is_safe",
        }
    }
}

impl AnnotationSection {
    fn fragment(&self) -> LeanFragment {
        LeanFragment { name: self.name.clone(), lean: self.lean.clone() }
    }
}

impl AnnotatedItemKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Function => "function",
            Self::Type => "type",
            Self::Trait => "trait",
            Self::Impl => "impl",
            Self::Global => "global",
        }
    }
}

impl AnnotatedItemId {
    fn new(kind: AnnotatedItemKind, index: usize) -> Self {
        Self { kind, index }
    }
}

fn path_segments(name: &Name) -> Vec<String> {
    name.name
        .iter()
        .map(|elem| match elem {
            PathElem::Ident(ident, _) => ident.clone(),
            PathElem::Impl(_) => "{impl}".to_string(),
            PathElem::Instantiated(_) => "{instantiated}".to_string(),
            PathElem::Target(target) => format!("{target:?}"),
        })
        .collect()
}

fn display_path(path: &[String]) -> String {
    if path.is_empty() { "<unknown>".to_string() } else { path.join("::") }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn doc_attr(json: &str) -> AttrInfo {
        AttrInfo {
            attributes: vec![Attribute::DocComment(format!("{METADATA_PREFIX}{json}"))],
            ..AttrInfo::default()
        }
    }

    fn annotation_json(kind: &str, sections: serde_json::Value) -> String {
        serde_json::json!({
            "version": 1,
            "kind": kind,
            "sections": sections,
        })
        .to_string()
    }

    fn function_facts(is_unsafe: bool, is_opaque: bool) -> ItemFacts {
        ItemFacts {
            kind: AnnotatedItemKind::Function,
            opacity: if is_opaque { ItemOpacity::Opaque } else { ItemOpacity::Transparent },
            is_unsafe_function: Some(is_unsafe),
            is_opaque_function: Some(is_opaque),
        }
    }

    fn section(kind: SectionKind, name: Option<&str>, lean: &str) -> AnnotationSection {
        AnnotationSection { kind, name: name.map(str::to_string), lean: lean.to_string() }
    }

    #[test]
    fn extracts_metadata_doc_comment() {
        let json = annotation_json(
            "spec",
            serde_json::json!([
                {"kind": "ensures", "name": "value", "lean": "ret = x + 1"},
                {"kind": "proof", "name": "value", "lean": "simp [incr]"}
            ]),
        );
        let annotation = extract_annotation(&doc_attr(&json), &["crate".into(), "incr".into()])
            .unwrap()
            .unwrap();

        assert_eq!(annotation.kind, AnnotationKind::Spec);
        assert_eq!(annotation.sections[0].kind, SectionKind::Ensures);
        assert_eq!(annotation.sections[0].name.as_deref(), Some("value"));
        assert_eq!(annotation.sections[0].lean, "ret = x + 1");
        assert_eq!(annotation.sections[1].lean, "simp [incr]");
    }

    #[test]
    fn rejects_duplicate_metadata_docs() {
        let json = annotation_json("spec", serde_json::json!([]));
        let attrs = AttrInfo {
            attributes: vec![
                Attribute::DocComment(format!("{METADATA_PREFIX}{json}")),
                Attribute::DocComment(format!("{METADATA_PREFIX}{json}")),
            ],
            ..AttrInfo::default()
        };
        let err = extract_annotation(&attrs, &["crate".into(), "f".into()]).unwrap_err();

        assert!(matches!(err, Error::MultipleAnnotations { .. }));
    }

    #[test]
    fn unsafe_axiom_must_be_opaque() {
        let annotation = Annotation {
            version: 1,
            kind: AnnotationKind::UnsafeAxiom,
            sections: vec![AnnotationSection {
                kind: SectionKind::Ensures,
                name: Some("value".into()),
                lean: "ret = 7".into(),
            }],
            argument_names: None,
        };
        let err = validate_annotation(
            &annotation,
            function_facts(true, false),
            &["crate".into(), "read".into()],
        )
        .unwrap_err();

        assert!(matches!(err, Error::UnsafeAxiomNotOpaque { .. }));
    }

    #[test]
    fn spec_must_not_be_opaque() {
        let annotation = Annotation {
            version: 1,
            kind: AnnotationKind::Spec,
            sections: vec![AnnotationSection {
                kind: SectionKind::Ensures,
                name: Some("value".into()),
                lean: "ret = 7".into(),
            }],
            argument_names: None,
        };
        let err = validate_annotation(
            &annotation,
            function_facts(true, true),
            &["crate".into(), "seven".into()],
        )
        .unwrap_err();

        assert!(matches!(err, Error::SpecIsOpaque { .. }));
    }

    #[test]
    fn requires_sections_require_unsafe_functions() {
        let annotation = Annotation {
            version: 1,
            kind: AnnotationKind::Spec,
            sections: vec![AnnotationSection {
                kind: SectionKind::Requires,
                name: Some("pre".into()),
                lean: "x > 0".into(),
            }],
            argument_names: None,
        };
        let err = validate_annotation(
            &annotation,
            function_facts(false, false),
            &["crate".into(), "f".into()],
        )
        .unwrap_err();

        assert!(matches!(err, Error::RequiresSafeFunction { .. }));
    }

    #[test]
    fn type_invariant_requires_is_valid_section() {
        let annotation = Annotation {
            version: 1,
            kind: AnnotationKind::TypeInvariant,
            sections: vec![],
            argument_names: None,
        };
        let err = validate_annotation(
            &annotation,
            ItemFacts::plain(AnnotatedItemKind::Type),
            &["crate".into(), "NonZero".into()],
        )
        .unwrap_err();

        assert!(matches!(err, Error::MissingRequiredSection { .. }));
    }

    #[test]
    fn rejects_annotation_on_wrong_item_kind() {
        let annotation = Annotation {
            version: 1,
            kind: AnnotationKind::Spec,
            sections: vec![],
            argument_names: None,
        };
        let err = validate_annotation(
            &annotation,
            ItemFacts::plain(AnnotatedItemKind::Type),
            &["crate".into(), "NotFn".into()],
        )
        .unwrap_err();

        assert!(matches!(err, Error::InvalidItemKind { .. }));
    }

    #[test]
    fn groups_function_sections_by_lean_insertion_point() {
        let annotation = Annotation {
            version: 1,
            kind: AnnotationKind::Spec,
            sections: vec![
                section(SectionKind::Context, None, "open scoped BigOperators"),
                section(SectionKind::Requires, Some("h_pre"), "x.val + y.val <= U32.max"),
                section(SectionKind::Ensures, Some("h_post"), "ret.val = x.val + y.val"),
                section(SectionKind::ProofContext, None, "have h_comm := Nat.add_comm x.val y.val"),
                section(SectionKind::Proof, Some("h_post"), "rw [h_comm]"),
            ],
            argument_names: None,
        };
        let AnnotationBody::Function(body) = annotation_body(&annotation, "crate::add").unwrap()
        else {
            panic!("expected function body");
        };

        assert_eq!(body.kind, FunctionAnnotationKind::Spec);
        assert_eq!(body.context[0].lean, "open scoped BigOperators");
        assert_eq!(body.requires[0].name.as_deref(), Some("h_pre"));
        assert_eq!(body.requires[0].lean, "x.val + y.val <= U32.max");
        assert_eq!(body.ensures[0].lean, "ret.val = x.val + y.val");
        assert_eq!(body.proof_context[0].lean, "have h_comm := Nat.add_comm x.val y.val");
        assert_eq!(body.proofs[0].lean, "rw [h_comm]");
    }

    #[test]
    fn rejects_invalid_section_for_annotation_kind() {
        let annotation = Annotation {
            version: 1,
            kind: AnnotationKind::UnsafeAxiom,
            sections: vec![section(SectionKind::Proof, Some("h_post"), "simp")],
            argument_names: None,
        };
        let err = validate_annotation(
            &annotation,
            function_facts(true, true),
            &["crate".into(), "axiom".into()],
        )
        .unwrap_err();

        assert!(matches!(err, Error::InvalidSection { .. }));
    }

    #[test]
    fn groups_type_invariant_sections() {
        let annotation = Annotation {
            version: 1,
            kind: AnnotationKind::TypeInvariant,
            sections: vec![
                section(SectionKind::Context, None, "open Anneal"),
                section(SectionKind::IsValid, None, "fun self => self.val > 0"),
            ],
            argument_names: None,
        };
        let AnnotationBody::TypeInvariant(body) =
            annotation_body(&annotation, "crate::NonZero").unwrap()
        else {
            panic!("expected type invariant body");
        };

        assert_eq!(body.context[0].lean, "open Anneal");
        assert_eq!(body.is_valid[0].lean, "fun self => self.val > 0");
    }

    #[test]
    fn manifest_serializes_grouped_annotation_body() {
        let target_kind = crate::resolve::AnnealTargetKind::Lib;
        let artifact = crate::scanner::AnnealArtifact {
            name: crate::resolve::AnnealTargetName {
                package_name: cargo_metadata::PackageName::new("test_proj".to_string()),
                target_name: "test_proj".to_string(),
                kind: target_kind,
            },
            target_kind,
            manifest_path: "/tmp/test_proj/Cargo.toml".into(),
        };
        let annotation = Annotation {
            version: 1,
            kind: AnnotationKind::Spec,
            sections: vec![section(SectionKind::Requires, Some("h_pre"), "x > 0")],
            argument_names: Some(vec!["x".into()]),
        };
        let body = validate_annotation(
            &annotation,
            function_facts(true, false),
            &["test_proj".into(), "f".into()],
        )
        .unwrap();
        let annotated = AnnotatedArtifact {
            artifact,
            items: vec![AnnotatedItem {
                item_id: AnnotatedItemId::new(AnnotatedItemKind::Function, 7),
                item_kind: AnnotatedItemKind::Function,
                path: vec!["test_proj".into(), "f".into()],
                annotation,
                body,
            }],
        };

        let value = serde_json::to_value(annotation_manifest(&annotated)).unwrap();

        assert_eq!(value["artifact"]["package_name"], "test_proj");
        assert_eq!(value["artifact"]["target_kind"], "Lib");
        assert!(value["artifact"]["llbc_file"].as_str().unwrap().ends_with(".llbc"));
        assert_eq!(value["items"][0]["item_id"]["kind"], "function");
        assert_eq!(value["items"][0]["item_id"]["index"], 7);
        assert_eq!(value["items"][0]["item_kind"], "function");
        assert_eq!(value["items"][0]["annotation"]["argument_names"], serde_json::json!(["x"]));
        assert_eq!(value["items"][0]["body"]["function"]["requires"][0]["name"], "h_pre");
        assert_eq!(value["items"][0]["body"]["function"]["requires"][0]["lean"], "x > 0");
    }
}
