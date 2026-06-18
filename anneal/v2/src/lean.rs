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

    let modules = generated_modules(artifacts);
    std::fs::write(generated_root.join("Generated.lean"), generated_imports(&modules))
        .with_context(|| {
            format!("failed to write `{}`", generated_root.join("Generated.lean").display())
        })?;

    std::fs::write(lean_root.join("lakefile.lean"), lakefile(toolchain, &modules)?).with_context(
        || format!("failed to write `{}`", lean_root.join("lakefile.lean").display()),
    )?;

    Ok(lean_root)
}

fn generated_modules(artifacts: &[crate::aeneas::LeanArtifact]) -> Vec<String> {
    let mut modules = vec!["Generated".to_string()];
    for artifact in artifacts {
        let slug = artifact.artifact.artifact_slug();
        for stem in ["Funs", "Types", "FunsExternal", "TypesExternal"] {
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
}
