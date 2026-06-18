// Copyright 2026 The Fuchsia Authors
//
// Licensed under the 2-Clause BSD License <LICENSE-BSD or
// https://opensource.org/license/bsd-2-clause>, Apache License, Version 2.0
// <LICENSE-APACHE or https://www.apache.org/licenses/LICENSE-2.0>, or the MIT
// license <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to
// those terms.

//! Minimal Lean workspace scaffolding for generated Aeneas output.

use anyhow::Context as _;

const ANNEAL_LEAN: &str = include_str!("../../src/Anneal.lean");

pub fn write_workspace(
    toolchain: &crate::setup::Toolchain,
    roots: &crate::resolve::LockedRoots<'_>,
    artifacts: &[crate::aeneas::LeanArtifact],
) -> anyhow::Result<std::path::PathBuf> {
    let lean_root = roots.lean_root();
    let generated_root = roots.lean_generated_root();
    std::fs::create_dir_all(&generated_root)
        .with_context(|| format!("failed to create `{}`", generated_root.display()))?;

    let lean_toolchain =
        std::fs::read_to_string(toolchain.aeneas_lean_dir().join("lean-toolchain"))
            .context("failed to read Aeneas lean-toolchain")?;
    std::fs::write(lean_root.join("lean-toolchain"), lean_toolchain).with_context(|| {
        format!("failed to write `{}`", lean_root.join("lean-toolchain").display())
    })?;
    std::fs::write(generated_root.join("Anneal.lean"), ANNEAL_LEAN).with_context(|| {
        format!("failed to write `{}`", generated_root.join("Anneal.lean").display())
    })?;

    let modules = generated_modules(artifacts);
    std::fs::write(generated_root.join("Generated.lean"), generated_imports(&modules))
        .with_context(|| {
            format!("failed to write `{}`", generated_root.join("Generated.lean").display())
        })?;

    std::fs::write(lean_root.join("lakefile.lean"), lakefile(toolchain, &modules)?).with_context(
        || format!("failed to write `{}`", lean_root.join("lakefile.lean").display()),
    )?;
    std::fs::write(lean_root.join("lake-manifest.json"), lake_manifest(toolchain)?).with_context(
        || format!("failed to write `{}`", lean_root.join("lake-manifest.json").display()),
    )?;

    Ok(lean_root)
}

fn generated_modules(artifacts: &[crate::aeneas::LeanArtifact]) -> Vec<String> {
    let mut modules = vec!["Generated".to_string(), "Anneal".to_string()];
    for artifact in artifacts {
        let slug = artifact.artifact.artifact_slug();
        for stem in ["Funs", "Types", "FunsExternal", "TypesExternal", "Specs"] {
            if artifact.output_dir.join(format!("{stem}.lean")).exists() {
                modules.push(format!("{slug}.{stem}"));
            }
        }
    }
    modules
}

fn generated_imports(modules: &[String]) -> String {
    let mut imports = String::new();
    for module in modules.iter().filter(|module| module.as_str() != "Generated") {
        imports.push_str("import ");
        imports.push_str(module);
        imports.push('\n');
    }
    imports
}

fn lakefile(toolchain: &crate::setup::Toolchain, modules: &[String]) -> anyhow::Result<String> {
    let roots = modules.iter().map(|module| format!("`{module}")).collect::<Vec<_>>().join(", ");
    let aeneas_path = lean_string(&toolchain.aeneas_lean_dir().to_string_lossy());
    Ok(format!(
        r#"import Lake
open Lake DSL

require aeneas from {aeneas_path}

package anneal_verification

@[default_target]
lean_lib Generated where
  srcDir := "generated"
  roots := #[{roots}]
"#
    ))
}

fn lake_manifest(toolchain: &crate::setup::Toolchain) -> anyhow::Result<String> {
    let aeneas_lean_dir = toolchain.aeneas_lean_dir();
    let manifest_path = aeneas_lean_dir.join("lake-manifest.json");
    let manifest: serde_json::Value = serde_json::from_reader(
        std::fs::File::open(&manifest_path)
            .with_context(|| format!("failed to open `{}`", manifest_path.display()))?,
    )
    .with_context(|| format!("failed to parse `{}`", manifest_path.display()))?;
    let aeneas_packages = manifest
        .get("packages")
        .and_then(serde_json::Value::as_array)
        .context("Aeneas Lake manifest is missing a packages array")?;

    let mut packages = vec![serde_json::json!({
        "type": "path",
        "name": "aeneas",
        "dir": aeneas_lean_dir,
        "inherited": false,
    })];
    for entry in aeneas_packages {
        let mut entry = entry
            .as_object()
            .cloned()
            .context("Aeneas Lake manifest package entry is not an object")?;
        let package_type =
            entry.get("type").and_then(serde_json::Value::as_str).with_context(|| {
                format!("Aeneas Lake manifest package entry is missing type: {entry:?}")
            })?;
        if package_type != "path" {
            anyhow::bail!("Aeneas Lake manifest package entry is {package_type:?}, not a path");
        }
        let package_dir =
            entry.get("dir").and_then(serde_json::Value::as_str).with_context(|| {
                format!("Aeneas Lake manifest package entry is missing dir: {entry:?}")
            })?;
        let package_dir = std::path::Path::new(package_dir);
        let package_dir = if package_dir.is_absolute() {
            package_dir.to_path_buf()
        } else {
            aeneas_lean_dir.join(package_dir)
        };
        entry.insert("dir".to_string(), serde_json::json!(package_dir));
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
    let mut json =
        serde_json::to_string_pretty(&manifest).context("failed to serialize Lake manifest")?;
    json.push('\n');
    Ok(json)
}

fn lean_string(value: &str) -> String {
    serde_json::to_string(value).expect("serializing a string cannot fail")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_imports_skip_root_module() {
        let imports = generated_imports(&[
            "Generated".to_string(),
            "Pkg.Funs".to_string(),
            "Pkg.Types".to_string(),
        ]);

        assert_eq!(imports, "import Pkg.Funs\nimport Pkg.Types\n");
    }

    #[test]
    fn lakefile_uses_generated_roots_and_aeneas_path() {
        let toolchain = crate::setup::Toolchain::for_test("/toolchain".into());
        let content =
            lakefile(&toolchain, &["Generated".to_string(), "Pkg.Funs".to_string()]).unwrap();

        assert!(content.contains("require aeneas from \"/toolchain/aeneas/backends/lean\""));
        assert!(content.contains("roots := #[`Generated, `Pkg.Funs]"));
    }

    #[test]
    fn lake_manifest_reuses_aeneas_archive_packages() {
        let temp = tempfile::tempdir().unwrap();
        let toolchain = crate::setup::Toolchain::for_test(temp.path().join("toolchain"));
        let aeneas_lean = toolchain.aeneas_lean_dir();
        std::fs::create_dir_all(&aeneas_lean).unwrap();
        std::fs::write(
            aeneas_lean.join("lake-manifest.json"),
            r#"{
  "packages": [
    {"type": "path", "name": "mathlib", "dir": "../packages/mathlib"}
  ]
}
"#,
        )
        .unwrap();

        let manifest = lake_manifest(&toolchain).unwrap();
        let value: serde_json::Value = serde_json::from_str(&manifest).unwrap();

        assert_eq!(value["packages"][0]["name"], "aeneas");
        assert_eq!(value["packages"][0]["inherited"], false);
        assert_eq!(value["packages"][1]["name"], "mathlib");
        assert_eq!(value["packages"][1]["inherited"], true);
        assert!(
            value["packages"][1]["dir"]
                .as_str()
                .unwrap()
                .ends_with("aeneas/backends/lean/../packages/mathlib")
        );
    }
}
