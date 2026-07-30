// Copyright 2026 The Fuchsia Authors
//
// Licensed under the 2-Clause BSD License <LICENSE-BSD or
// https://opensource.org/license/bsd-2-clause>, Apache License, Version 2.0
// <LICENSE-APACHE or https://www.apache.org/licenses/LICENSE-2.0>, or the MIT
// license <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to
// those terms.

//! Selection of the Rust source compiled by rustc and Charon.
//!
//! `#[anneal]` is a real procedural macro. Charon therefore compiles the user's
//! original manifest and observes the same macro expansion as rustc. In
//! particular, Anneal no longer reparses and pretty-prints a derived workspace,
//! which used to destroy the user's line and column coordinates.

/// The source selected for Charon.
pub struct SourceView;

impl SourceView {
    /// Selects the original Cargo manifests.
    pub fn prepare(
        _roots: &crate::resolve::LockedRoots<'_>,
        _packages: &[crate::scanner::AnnealArtifact],
    ) -> anyhow::Result<Self> {
        Ok(Self)
    }

    /// Returns the artifact's original manifest.
    pub fn manifest_path<'a>(
        &'a self,
        artifact: &'a crate::scanner::AnnealArtifact,
    ) -> std::borrow::Cow<'a, std::path::Path> {
        std::borrow::Cow::Borrowed(&artifact.manifest_path)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn source_view_returns_the_original_manifest_without_touching_source() {
        let temp = tempfile::tempdir().unwrap();
        let manifest = temp.path().join("Cargo.toml");
        let source_path = temp.path().join("src/lib.rs");
        std::fs::create_dir_all(source_path.parent().unwrap()).unwrap();
        std::fs::write(&manifest, "[package]\nname = \"source-view\"\n").unwrap();
        let source = "#[anneal(spec(requires(prop = flag)))]\nfn f(flag: bool) {}\n";
        std::fs::write(&source_path, source).unwrap();
        let artifact = crate::scanner::AnnealArtifact {
            name: crate::resolve::AnnealTargetName {
                package_name: serde_json::from_str("\"source-view\"").unwrap(),
                target_name: "source_view".into(),
                kind: crate::resolve::AnnealTargetKind::Lib,
            },
            target_kind: crate::resolve::AnnealTargetKind::Lib,
            manifest_path: manifest.clone(),
            src_path: Some(source_path.clone()),
        };

        let view = super::SourceView;
        assert!(matches!(
            view.manifest_path(&artifact),
            std::borrow::Cow::Borrowed(path) if path == manifest
        ));
        assert_eq!(std::fs::read_to_string(source_path).unwrap(), source);
    }
}
