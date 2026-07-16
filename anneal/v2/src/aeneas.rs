// Copyright 2026 The Fuchsia Authors
//
// Licensed under the 2-Clause BSD License <LICENSE-BSD or
// https://opensource.org/license/bsd-2-clause>, Apache License, Version 2.0
// <LICENSE-APACHE or https://www.apache.org/licenses/LICENSE-2.0>, or the MIT
// license <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to
// those terms.

//! Orchestration of Aeneas lowering from LLBC to Lean.

use anyhow::Context as _;
use rayon::prelude::{IntoParallelRefIterator as _, ParallelIterator as _};

/// Runs Aeneas over all generated LLBC artifacts.
pub fn run_aeneas(
    toolchain: &crate::setup::Toolchain,
    roots: &crate::resolve::LockedRoots,
    packages: &[crate::scanner::AnnealArtifact],
    show_progress: bool,
) -> anyhow::Result<()> {
    let lean_root = roots.lean_root();
    std::fs::create_dir_all(&lean_root).context("Failed to create Lean output directory")?;

    let print_mutex = std::sync::Arc::new(std::sync::Mutex::new(()));
    let mp = if show_progress {
        Some(std::sync::Arc::new(indicatif::MultiProgress::new()))
    } else {
        None
    };

    packages.par_iter().try_for_each(|artifact| {
        let pb = mp.as_ref().map(|m| {
            let pb = m.add(indicatif::ProgressBar::new_spinner());
            pb.set_style(
                indicatif::ProgressStyle::default_spinner()
                    .template("{spinner:.green} {msg}")
                    .unwrap(),
            );
            pb.set_message(format!("Lowering {}...", artifact.name.package_name));
            pb
        });

        let llbc_path = artifact.llbc_path(roots);
        let lean_dir = artifact.lean_dir_path(roots);
        if lean_dir.exists() {
            std::fs::remove_dir_all(&lean_dir)
                .with_context(|| format!("Failed to clear {}", lean_dir.display()))?;
        }
        std::fs::create_dir_all(&lean_dir)
            .with_context(|| format!("Failed to create {}", lean_dir.display()))?;

        log::info!("Invoking Aeneas on '{}': {}", artifact.name.package_name, llbc_path.display());

        let mut cmd = toolchain.command(crate::setup::Tool::Aeneas)?;
        configure_aeneas_command(&mut cmd, &llbc_path, &lean_dir);

        log::debug!("Executing aeneas command: {:?}", cmd);

        let output = crate::util::run_command_with_progress(cmd, pb.clone(), |line, pb| {
            if let Some(pb) = pb {
                pb.set_message(line.to_string());
            }
            log::debug!("aeneas: {line}");
            Ok(())
        })?;

        if !output.status.success() {
            let _lock = print_mutex.lock().unwrap();
            eprintln!("=== Aeneas failure output for '{}' ===", artifact.name.package_name);
            for line in output.stderr_lines {
                eprintln!("{line}");
            }
            anyhow::bail!("Aeneas failed with status: {}", output.status);
        }

        ensure_split_lean_output(&lean_dir)?;
        ensure_translation_sidecar(&lean_dir)?;
        Ok(())
    })?;

    Ok(())
}

fn configure_aeneas_command(
    cmd: &mut std::process::Command,
    llbc_path: &std::path::Path,
    lean_dir: &std::path::Path,
) {
    cmd.arg("-backend")
        .arg("lean")
        .arg("-dest")
        .arg(lean_dir)
        .arg("-split-files")
        .arg("-abort-on-error")
        .arg("-emit-json")
        .arg(llbc_path);
}

fn ensure_split_lean_output(lean_dir: &std::path::Path) -> anyhow::Result<()> {
    let mut has_lean = false;
    for entry in std::fs::read_dir(lean_dir)
        .with_context(|| format!("Failed to read {}", lean_dir.display()))?
    {
        let path = entry?.path();
        if path.extension().is_some_and(|ext| ext == "lean") {
            has_lean = true;
            break;
        }
    }

    if has_lean {
        Ok(())
    } else {
        anyhow::bail!("Aeneas did not emit any .lean files into {}", lean_dir.display())
    }
}

fn ensure_translation_sidecar(lean_dir: &std::path::Path) -> anyhow::Result<()> {
    let sidecar = lean_dir.join("translation.json");
    if sidecar.exists() {
        Ok(())
    } else {
        anyhow::bail!("Aeneas did not emit {}", sidecar.display())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn configure_aeneas_command_uses_split_lean_backend() {
        let mut cmd = std::process::Command::new("aeneas");
        configure_aeneas_command(
            &mut cmd,
            std::path::Path::new("/tmp/demo.llbc"),
            std::path::Path::new("/tmp/generated"),
        );

        let args = cmd.get_args().map(|arg| arg.to_string_lossy().into_owned()).collect::<Vec<_>>();
        assert_eq!(
            args,
            vec![
                "-backend",
                "lean",
                "-dest",
                "/tmp/generated",
                "-split-files",
                "-abort-on-error",
                "-emit-json",
                "/tmp/demo.llbc",
            ]
        );
    }
}
