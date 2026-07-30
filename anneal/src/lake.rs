// Copyright 2026 The Fuchsia Authors
//
// Licensed under the 2-Clause BSD License <LICENSE-BSD or
// https://opensource.org/license/bsd-2-clause>, Apache License, Version 2.0
// <LICENSE-APACHE or https://www.apache.org/licenses/LICENSE-2.0>, or the MIT
// license <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to
// those terms.

//! Lake workspace generation for Aeneas-emitted Lean files.

use anyhow::Context as _;

const AXIOM_AUDIT_SOURCE: &str = include_str!("../lean/AnnealAxiomAudit.lean");

/// Writes the Lake workspace metadata for generated Lean artifacts.
pub fn prepare_workspace(
    toolchain: &crate::setup::Toolchain,
    roots: &crate::resolve::LockedRoots,
    packages: &[crate::scanner::AnnealArtifact],
) -> anyhow::Result<()> {
    let workspace = roots.lean_root();
    std::fs::create_dir_all(&workspace)
        .with_context(|| format!("Failed to create {}", workspace.display()))?;

    let mut imports = Vec::new();
    for artifact in packages {
        let artifact_dir = artifact.lean_dir_path(roots);
        install_external_templates(&artifact_dir)?;
        imports.extend(module_imports(&workspace, &artifact_dir)?);
    }
    imports.sort();
    imports.dedup();

    let generated = imports.iter().map(|module| format!("import {module}\n")).collect::<String>();
    std::fs::write(workspace.join("Generated.lean"), generated).with_context(|| {
        format!("Failed to write {}", workspace.join("Generated.lean").display())
    })?;

    write_lakefile(&workspace, &toolchain.aeneas_lean_dir(), &imports)?;
    copy_workspace_file(
        &toolchain.aeneas_lean_dir().join("lean-toolchain"),
        &workspace.join("lean-toolchain"),
    )?;
    write_lake_manifest(&workspace, &toolchain.aeneas_lean_dir())?;

    Ok(())
}

/// Builds the generated Lake workspace.
pub fn build_workspace(
    toolchain: &crate::setup::Toolchain,
    roots: &crate::resolve::LockedRoots,
    packages: &[crate::scanner::AnnealArtifact],
    allow_sorry: bool,
    show_progress: bool,
) -> anyhow::Result<()> {
    let workspace = roots.lean_root();
    let pb = show_progress.then(|| {
        let pb = indicatif::ProgressBar::new_spinner();
        pb.set_style(
            indicatif::ProgressStyle::default_spinner().template("{spinner:.green} {msg}").unwrap(),
        );
        pb.set_message("Building generated Lean...");
        pb
    });

    let mut cmd = toolchain.command(crate::setup::Tool::Lake)?;
    cmd.current_dir(&workspace).args(["--keep-toolchain", "--old", "build", "Generated"]);

    let mut stdout_lines = Vec::new();
    let output = crate::util::run_command_with_progress(cmd, pb, |line, pb| {
        stdout_lines.push(line.to_string());
        if let Some(pb) = pb {
            pb.set_message(line.to_string());
        }
        log::debug!("lake: {line}");
        Ok(())
    })?;

    if !output.status.success() {
        eprintln!("=== Lake failure output ===");
        for line in stdout_lines {
            eprintln!("{line}");
        }
        for line in output.stderr_lines {
            eprintln!("{line}");
        }
        anyhow::bail!("Lake failed with status: {}", output.status);
    }

    audit_soundness_axioms(toolchain, roots, packages, allow_sorry, show_progress)
}

/// Loads the compiled soundness modules in a fixed checker and rejects
/// declarations that extended Lean's trusted axiom base.
///
/// The checker itself imports only Lean. It loads each already-compiled
/// `Soundness` olean at runtime, with imported environment extensions disabled,
/// and keys declarations by their recorded module id rather than namespace.
/// Consequently proof metaprogramming cannot evade the audit merely by adding
/// an axiom under an unrelated name.
fn audit_soundness_axioms(
    toolchain: &crate::setup::Toolchain,
    roots: &crate::resolve::LockedRoots,
    packages: &[crate::scanner::AnnealArtifact],
    allow_sorry: bool,
    show_progress: bool,
) -> anyhow::Result<()> {
    let workspace = roots.lean_root();
    let mut modules = packages
        .iter()
        .filter_map(|artifact| {
            let soundness = artifact.lean_dir_path(roots).join("Soundness.lean");
            soundness.is_file().then(|| module_name_from_path(&workspace, &soundness))
        })
        .collect::<anyhow::Result<Vec<_>>>()?;
    modules.sort();
    modules.dedup();
    if modules.is_empty() {
        return Ok(());
    }

    // Write this after Lake finishes so proof metaprogramming cannot leave a
    // modified source file for the checker to compile.
    let audit_path = workspace.join("AnnealAxiomAudit.lean");
    std::fs::write(&audit_path, AXIOM_AUDIT_SOURCE)
        .with_context(|| format!("Failed to write {}", audit_path.display()))?;

    let pb = show_progress.then(|| {
        let pb = indicatif::ProgressBar::new_spinner();
        pb.set_style(
            indicatif::ProgressStyle::default_spinner().template("{spinner:.green} {msg}").unwrap(),
        );
        pb.set_message("Auditing generated Lean axioms...");
        pb
    });

    let mut cmd = toolchain.command(crate::setup::Tool::Lake)?;
    cmd.current_dir(&workspace)
        .args(["--keep-toolchain", "--old", "env", "lean", "--run"])
        .arg(&audit_path);
    if allow_sorry {
        cmd.arg("--allow-sorry");
    }
    cmd.args(&modules);

    let mut stdout_lines = Vec::new();
    let output = crate::util::run_command_with_progress(cmd, pb, |line, pb| {
        stdout_lines.push(line.to_string());
        if let Some(pb) = pb {
            pb.set_message(line.to_string());
        }
        log::debug!("Lean axiom audit: {line}");
        Ok(())
    })?;
    if output.status.success() {
        Ok(())
    } else {
        eprintln!("=== Lean axiom audit failure output ===");
        for line in stdout_lines {
            eprintln!("{line}");
        }
        for line in output.stderr_lines {
            eprintln!("{line}");
        }
        anyhow::bail!("Lean axiom audit failed with status: {}", output.status)
    }
}

fn install_external_templates(artifact_dir: &std::path::Path) -> anyhow::Result<()> {
    for entry in std::fs::read_dir(artifact_dir)
        .with_context(|| format!("Failed to read {}", artifact_dir.display()))?
    {
        let path = entry?.path();
        let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        let Some(stem) = file_name.strip_suffix("_Template.lean") else {
            continue;
        };
        let target = artifact_dir.join(format!("{stem}.lean"));
        if !target.exists() {
            copy_workspace_file(&path, &target)?;
        }
    }
    Ok(())
}

fn copy_workspace_file(source: &std::path::Path, target: &std::path::Path) -> anyhow::Result<()> {
    if target.exists() {
        std::fs::remove_file(target)
            .with_context(|| format!("Failed to remove {}", target.display()))?;
    }
    std::fs::copy(source, target)
        .with_context(|| format!("Failed to copy {} to {}", source.display(), target.display()))?;
    make_writable(target)
}

fn make_writable(path: &std::path::Path) -> anyhow::Result<()> {
    let mut permissions = std::fs::metadata(path)
        .with_context(|| format!("Failed to inspect {}", path.display()))?
        .permissions();
    if permissions.readonly() {
        permissions.set_readonly(false);
        std::fs::set_permissions(path, permissions)
            .with_context(|| format!("Failed to make {} writable", path.display()))?;
    }
    Ok(())
}

fn module_imports(
    workspace: &std::path::Path,
    artifact_dir: &std::path::Path,
) -> anyhow::Result<Vec<String>> {
    let mut imports = Vec::new();
    for entry in std::fs::read_dir(artifact_dir)
        .with_context(|| format!("Failed to read {}", artifact_dir.display()))?
    {
        let path = entry?.path();
        if path.extension().is_some_and(|ext| ext == "lean")
            && !path
                .file_stem()
                .and_then(|stem| stem.to_str())
                .is_some_and(|stem| stem.ends_with("_Template"))
        {
            imports.push(module_name_from_path(workspace, &path)?);
        }
    }
    Ok(imports)
}

fn module_name_from_path(
    workspace: &std::path::Path,
    path: &std::path::Path,
) -> anyhow::Result<String> {
    let relative = path
        .strip_prefix(workspace)
        .with_context(|| format!("{} is not under {}", path.display(), workspace.display()))?;
    let module = relative.with_extension("");
    let components = module
        .components()
        .map(|component| component.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    Ok(components.join("."))
}

fn write_lakefile(
    workspace: &std::path::Path,
    aeneas_lean: &std::path::Path,
    imports: &[String],
) -> anyhow::Result<()> {
    let roots = lake_roots(imports);
    let contents = format!(
        r#"import Lake
open Lake DSL

require aeneas from "{}"

package «anneal_verification»

@[default_target]
lean_lib «Generated» where
  srcDir := "."
  roots := #[{}]
"#,
        lake_string(aeneas_lean),
        roots.join(", ")
    );
    std::fs::write(workspace.join("lakefile.lean"), contents)
        .with_context(|| format!("Failed to write {}", workspace.join("lakefile.lean").display()))
}

fn lake_roots(imports: &[String]) -> Vec<String> {
    let mut roots = vec!["`Generated".to_string()];
    roots.extend(imports.iter().map(|module| format!("`{module}")));
    roots.sort();
    roots.dedup();
    roots
}

fn write_lake_manifest(
    workspace: &std::path::Path,
    aeneas_lean: &std::path::Path,
) -> anyhow::Result<()> {
    let aeneas_lean = std::fs::canonicalize(aeneas_lean)
        .with_context(|| format!("Failed to canonicalize {}", aeneas_lean.display()))?;
    let workspace = std::fs::canonicalize(workspace)
        .with_context(|| format!("Failed to canonicalize {}", workspace.display()))?;
    let manifest_path = aeneas_lean.join("lake-manifest.json");
    let manifest: serde_json::Value = serde_json::from_reader(
        std::fs::File::open(&manifest_path)
            .with_context(|| format!("Failed to open {}", manifest_path.display()))?,
    )
    .with_context(|| format!("Failed to parse {}", manifest_path.display()))?;

    let aeneas_packages =
        manifest.get("packages").and_then(serde_json::Value::as_array).ok_or_else(|| {
            anyhow::anyhow!("Aeneas Lake manifest {} is missing packages", manifest_path.display())
        })?;

    let mut packages = vec![serde_json::json!({
        "type": "path",
        "name": "aeneas",
        "dir": relative_manifest_string(&aeneas_lean, &workspace)?,
        "inherited": false,
    })];

    for entry in aeneas_packages {
        let mut entry = entry.as_object().cloned().ok_or_else(|| {
            anyhow::anyhow!("Aeneas Lake manifest package entry is not an object")
        })?;
        let package_type = entry
            .get("type")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| anyhow::anyhow!("Aeneas Lake manifest package entry is missing type"))?;
        if package_type != "path" {
            anyhow::bail!(
                "Aeneas Lake manifest package entry is {package_type:?}, not a path dependency"
            );
        }
        let package_dir = entry
            .get("dir")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| anyhow::anyhow!("Aeneas Lake manifest package entry is missing dir"))?;
        let package_dir = std::path::Path::new(package_dir);
        let package_dir = if package_dir.is_absolute() {
            package_dir.to_path_buf()
        } else {
            aeneas_lean.join(package_dir)
        };
        let package_dir = std::fs::canonicalize(&package_dir)
            .with_context(|| format!("Failed to canonicalize {}", package_dir.display()))?;
        entry.insert(
            "dir".to_string(),
            serde_json::json!(relative_manifest_string(&package_dir, &workspace)?),
        );
        entry.insert("inherited".to_string(), serde_json::json!(true));
        packages.push(serde_json::Value::Object(entry));
    }

    let manifest = serde_json::json!({
        "version": "1.2.0",
        "packagesDir": ".lake/packages",
        "packages": packages,
        "name": "anneal_verification",
        "lakeDir": ".lake",
        "fixedToolchain": false,
    });
    std::fs::write(
        workspace.join("lake-manifest.json"),
        format!("{}\n", serde_json::to_string_pretty(&manifest)?),
    )
    .with_context(|| format!("Failed to write {}", workspace.join("lake-manifest.json").display()))
}

fn relative_manifest_string(
    path: &std::path::Path,
    base: &std::path::Path,
) -> anyhow::Result<String> {
    let path = pathdiff::diff_paths(path, base).ok_or_else(|| {
        anyhow::anyhow!(
            "failed to compute relative path from {} to {}",
            base.display(),
            path.display()
        )
    })?;
    Ok(path.to_string_lossy().into_owned())
}

fn lake_string(path: &std::path::Path) -> String {
    path.to_string_lossy().replace('\\', "\\\\").replace('"', "\\\"")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn module_name_from_path_uses_relative_lean_path() {
        let workspace = std::path::Path::new("/tmp/ws");
        let path = std::path::Path::new("/tmp/ws/TestProj/Funs.lean");

        assert_eq!(module_name_from_path(workspace, path).unwrap(), "TestProj.Funs");
    }

    #[test]
    fn module_imports_ignore_templates() {
        let temp = tempfile::tempdir().unwrap();
        let artifact = temp.path().join("Demo");
        std::fs::create_dir_all(&artifact).unwrap();
        std::fs::write(artifact.join("Types.lean"), "").unwrap();
        std::fs::write(artifact.join("FunsExternal_Template.lean"), "").unwrap();

        assert_eq!(module_imports(temp.path(), &artifact).unwrap(), vec!["Demo.Types"]);
    }

    #[test]
    fn lake_roots_include_generated_and_imported_modules() {
        let imports = vec![
            "Demo.Funs".to_string(),
            "Demo.Soundness".to_string(),
            "Demo.Types".to_string(),
            "Demo.Funs".to_string(),
        ];

        assert_eq!(
            lake_roots(&imports),
            vec!["`Demo.Funs", "`Demo.Soundness", "`Demo.Types", "`Generated"]
        );
    }

    #[test]
    fn install_external_templates_creates_missing_external_file() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(temp.path().join("FunsExternal_Template.lean"), "import Aeneas\n").unwrap();

        install_external_templates(temp.path()).unwrap();

        assert_eq!(
            std::fs::read_to_string(temp.path().join("FunsExternal.lean")).unwrap(),
            "import Aeneas\n"
        );
    }

    #[test]
    fn copy_workspace_file_replaces_readonly_destination() {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("source");
        let target = temp.path().join("target");
        std::fs::write(&source, "new").unwrap();
        std::fs::write(&target, "old").unwrap();
        let mut permissions = std::fs::metadata(&target).unwrap().permissions();
        permissions.set_readonly(true);
        std::fs::set_permissions(&target, permissions).unwrap();

        copy_workspace_file(&source, &target).unwrap();

        assert_eq!(std::fs::read_to_string(&target).unwrap(), "new");
        assert!(!std::fs::metadata(&target).unwrap().permissions().readonly());
    }
}
