// Copyright 2026 The Fuchsia Authors
//
// Licensed under the 2-Clause BSD License <LICENSE-BSD or
// https://opensource.org/license/bsd-2-clause>, Apache License, Version 2.0
// <LICENSE-APACHE or https://www.apache.org/licenses/LICENSE-2.0>, or the MIT
// license <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to
// those terms.

//! Orchestration of Aeneas Lean extraction from LLBC artifacts.

use anyhow::{Context as _, bail};

#[derive(Debug, Clone)]
pub struct LeanArtifact {
    pub artifact: crate::scanner::AnnealArtifact,
    pub output_dir: std::path::PathBuf,
}

pub fn run_aeneas(
    toolchain: &crate::setup::Toolchain,
    roots: &crate::resolve::LockedRoots<'_>,
    artifacts: &[crate::scanner::AnnealArtifact],
) -> anyhow::Result<Vec<LeanArtifact>> {
    let lean_generated_root = roots.lean_generated_root();
    std::fs::create_dir_all(&lean_generated_root)
        .with_context(|| format!("failed to create `{}`", lean_generated_root.display()))?;

    artifacts.iter().map(|artifact| run_artifact(toolchain, roots, artifact)).collect()
}

fn run_artifact(
    toolchain: &crate::setup::Toolchain,
    roots: &crate::resolve::LockedRoots<'_>,
    artifact: &crate::scanner::AnnealArtifact,
) -> anyhow::Result<LeanArtifact> {
    let output_dir = roots.lean_generated_root().join(artifact.artifact_slug());
    if output_dir.exists() {
        std::fs::remove_dir_all(&output_dir)
            .with_context(|| format!("failed to remove stale `{}`", output_dir.display()))?;
    }
    std::fs::create_dir_all(&output_dir)
        .with_context(|| format!("failed to create `{}`", output_dir.display()))?;

    let llbc_path = artifact.llbc_path(roots);
    let mut cmd = toolchain.command(crate::setup::Tool::Aeneas)?;
    cmd.args(["-backend", "lean"])
        .arg("-dest")
        .arg(&output_dir)
        .args(["-split-files", "-abort-on-error"])
        .arg(&llbc_path);

    log::debug!("Executing Aeneas command: {:?}", cmd);
    let output = cmd.output().context("failed to run Aeneas")?;
    if !output.status.success() {
        bail!(
            "Aeneas failed for `{}` with status {}\nstdout:\n{}\nstderr:\n{}",
            artifact.name.target_name,
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    materialize_external_templates(&output_dir)?;

    Ok(LeanArtifact { artifact: artifact.clone(), output_dir })
}

fn materialize_external_templates(output_dir: &std::path::Path) -> anyhow::Result<()> {
    for stem in ["FunsExternal", "TypesExternal"] {
        let template = output_dir.join(format!("{stem}_Template.lean"));
        let concrete = output_dir.join(format!("{stem}.lean"));
        if template.exists() && !concrete.exists() {
            std::fs::copy(&template, &concrete).with_context(|| {
                format!("failed to copy `{}` to `{}`", template.display(), concrete.display())
            })?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn materializes_missing_external_template_modules() {
        let temp = tempfile::tempdir().unwrap();
        let output_dir = temp.path();
        std::fs::write(output_dir.join("FunsExternal_Template.lean"), "-- funs\n").unwrap();
        std::fs::write(output_dir.join("TypesExternal_Template.lean"), "-- types\n").unwrap();

        materialize_external_templates(output_dir).unwrap();

        assert_eq!(
            std::fs::read_to_string(output_dir.join("FunsExternal.lean")).unwrap(),
            "-- funs\n"
        );
        assert_eq!(
            std::fs::read_to_string(output_dir.join("TypesExternal.lean")).unwrap(),
            "-- types\n"
        );
    }

    #[test]
    fn preserves_existing_external_modules() {
        let temp = tempfile::tempdir().unwrap();
        let output_dir = temp.path();
        std::fs::write(output_dir.join("FunsExternal_Template.lean"), "-- template\n").unwrap();
        std::fs::write(output_dir.join("FunsExternal.lean"), "-- user model\n").unwrap();

        materialize_external_templates(output_dir).unwrap();

        assert_eq!(
            std::fs::read_to_string(output_dir.join("FunsExternal.lean")).unwrap(),
            "-- user model\n"
        );
    }
}
