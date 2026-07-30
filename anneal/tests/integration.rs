// Copyright 2026 The Fuchsia Authors
//
// Licensed under the 2-Clause BSD License <LICENSE-BSD or
// https://opensource.org/license/bsd-2-clause>, Apache License, Version 2.0
// <LICENSE-APACHE or https://www.apache.org/licenses/LICENSE-2.0>, or the MIT
// license <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to
// those terms.

#[cfg(feature = "exocrate_tests")]
fn cargo_anneal_bin_path() -> std::path::PathBuf {
    std::env::var("CARGO_BIN_EXE_cargo-anneal")
        .or_else(|_| std::env::var("CARGO_BIN_EXE_cargo_anneal"))
        .expect("CARGO_BIN_EXE_* not set")
        .into()
}

#[cfg(feature = "exocrate_tests")]
fn cargo_anneal_command(bin_path: &std::path::Path) -> std::process::Command {
    let mut cmd = std::process::Command::new(bin_path);
    let home = std::env::var_os("HOME").unwrap_or_else(|| {
        let home = std::env::temp_dir().join("cargo-anneal-test-home");
        std::fs::create_dir_all(&home).expect("failed to create test HOME");
        home.into_os_string()
    });
    cmd.env_clear()
        .env("__ANNEAL_LOCAL_DEV", "1")
        .env("CARGO_MANIFEST_DIR", env!("CARGO_MANIFEST_DIR"))
        .env("HOME", home);
    cmd
}

#[cfg(feature = "exocrate_tests")]
fn ensure_test_toolchain(bin_path: &std::path::Path) {
    static SETUP_RESULT: std::sync::OnceLock<Result<(), String>> = std::sync::OnceLock::new();

    let result = SETUP_RESULT.get_or_init(|| {
        let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let mut cmd = cargo_anneal_command(bin_path);
        let output = cmd
            .arg("setup")
            .arg("--local-archive")
            .arg(manifest_dir.join("target/anneal-exocrate.tar.zst"))
            .output()
            .map_err(|err| format!("failed to execute cargo-anneal setup: {err}"))?;

        if output.status.success() {
            return Ok(());
        }

        Err(format!(
            "cargo-anneal setup failed\nstdout: {}\nstderr: {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        ))
    });

    if let Err(err) = result {
        panic!("{err}");
    }
}

#[cfg(feature = "exocrate_tests")]
#[test]
fn test_generate_subcommand_simple() {
    let temp_dir = tempfile::tempdir().unwrap();
    let project_dir = temp_dir.path().join("project");
    let output_dir = temp_dir.path().join("llbc_out");
    let lean_output_dir = temp_dir.path().join("lean_out");
    std::fs::create_dir_all(project_dir.join("src")).unwrap();
    std::fs::write(
        project_dir.join("Cargo.toml"),
        r#"
            [package]
            name = "test_proj"
            version = "0.1.0"
            edition = "2021"

            [lib]
            path = "src/lib.rs"
        "#,
    )
    .unwrap();
    std::fs::write(
        project_dir.join("src").join("lib.rs"),
        r#"
            pub fn identity(value: usize) -> usize {
                value
            }
        "#,
    )
    .unwrap();

    let bin_path = cargo_anneal_bin_path();
    ensure_test_toolchain(&bin_path);

    let mut cmd = cargo_anneal_command(&bin_path);
    if let Some(path) = std::env::var_os("PATH") {
        cmd.env("PATH", path);
    }
    cmd.arg("generate")
        .arg("--manifest-path")
        .arg(project_dir.join("Cargo.toml"))
        .arg("--lib")
        .arg("--output-dir")
        .arg(&output_dir)
        .arg("--lean-output-dir")
        .arg(&lean_output_dir);
    cmd.arg("--no-progress");

    let output = cmd.output().expect("failed to execute cargo-anneal");

    println!("stdout: {}", String::from_utf8_lossy(&output.stdout));
    println!("stderr: {}", String::from_utf8_lossy(&output.stderr));

    assert!(output.status.success(), "cargo-anneal failed");

    let mut found_llbc = false;
    if output_dir.exists() {
        for entry in std::fs::read_dir(&output_dir).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            if path.is_file() && path.extension().map_or(false, |ext| ext == "llbc") {
                found_llbc = true;
                break;
            }
        }
    }

    assert!(found_llbc, "No .llbc file found in output directory {:?}", output_dir);

    let mut found_lean = false;
    if lean_output_dir.exists() {
        for entry in std::fs::read_dir(&lean_output_dir).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            if path.is_dir()
                && std::fs::read_dir(&path)
                    .unwrap()
                    .any(|entry| entry.unwrap().path().extension().is_some_and(|ext| ext == "lean"))
            {
                found_lean = true;
                break;
            }
        }
    }

    assert!(found_lean, "No generated .lean files found in {:?}", lean_output_dir);
    assert!(lean_output_dir.join("Generated.lean").is_file());
    assert!(lean_output_dir.join("lakefile.lean").is_file());
    assert!(lean_output_dir.join("lake-manifest.json").is_file());
    assert!(lean_output_dir.join("lean-toolchain").is_file());
}

#[cfg(feature = "exocrate_tests")]
#[test]
fn test_generate_checks_call_site_preconditions() {
    let temp_dir = tempfile::tempdir().unwrap();
    let good_project = temp_dir.path().join("good_project");
    let bad_project = temp_dir.path().join("bad_project");
    write_call_site_project(
        &good_project,
        r#"
            #[anneal(spec(requires(name = "flag_true", prop = flag == true)))]
            pub fn needs_true(flag: bool) -> bool { flag }

            pub fn forwards(flag: bool) -> bool {
                if flag {
                    needs_true(flag)
                } else {
                    false
                }
            }

            #[anneal(
                spec(requires(name = "self_equal", prop = value == value)),
                proof(lean::tactic = "unfold identity_with_proof\nstep*"),
            )]
            pub fn identity_with_proof(value: u32) -> u32 { value }
        "#,
    );
    write_call_site_project(
        &bad_project,
        r#"
            #[anneal(spec(requires::name = "flag_true", requires::prop = flag == true))]
            pub fn needs_true(flag: bool) -> bool { flag }

            pub fn forwards(flag: bool) -> bool { needs_true(flag) }
        "#,
    );

    let bin_path = cargo_anneal_bin_path();
    ensure_test_toolchain(&bin_path);

    let good_lean = temp_dir.path().join("good_lean");
    let good_output =
        generate_project(&bin_path, &good_project, &temp_dir.path().join("good_llbc"), &good_lean);
    assert!(
        good_output.status.success(),
        "good call-site project failed\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&good_output.stdout),
        String::from_utf8_lossy(&good_output.stderr)
    );
    let good_soundness = generated_soundness(&good_lean);
    let strong = good_soundness.find("theorem identity_with_proof_soundness").unwrap();
    let diagnostic =
        good_soundness.find("theorem identity_with_proof_body_obligations_soundness").unwrap();
    assert!(strong < diagnostic, "explicit proof theorem must precede its derived diagnostic");
    assert!(good_soundness.contains("(required_pre_self_equal' : "));
    assert!(good_soundness.contains("WP.spec_mono (identity_with_proof_soundness value"));

    let bad_lean = temp_dir.path().join("bad_lean");
    let bad_output =
        generate_project(&bin_path, &bad_project, &temp_dir.path().join("bad_llbc"), &bad_lean);
    assert!(
        !bad_output.status.success(),
        "bad call-site project unexpectedly passed\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&bad_output.stdout),
        String::from_utf8_lossy(&bad_output.stderr)
    );
    let soundness = generated_soundness(&bad_lean);
    assert!(!soundness.contains("def needs_true_pre_flag_true"));
    assert!(soundness.contains("anneal__needs_true"));
    assert!(soundness.contains("flag_true"));
    assert!(soundness.contains("⦃ holds => holds = true ⦄"));
    assert!(soundness.contains("theorem needs_true_soundness"));
    assert!(soundness.contains("(required_pre_flag_true' : "));
    assert!(soundness.contains("needs_true flag ⦃ result => True ⦄"));
    assert!(soundness.contains(
        "theorem forwards_soundness\n    (flag : Bool) :\n  forwards flag ⦃ result => True ⦄"
    ));
    assert!(soundness.contains(
        "theorem forwards_body_obligations_soundness\n    (flag : Bool) :\n  forwards flag ⦃ result => True ⦄"
    ));
    assert!(!soundness.contains("anneal_callsite"));
}

#[cfg(feature = "exocrate_tests")]
#[test]
fn test_generate_requires_allow_sorry_for_an_incomplete_default_proof() {
    let temp_dir = tempfile::tempdir().unwrap();
    let project = temp_dir.path().join("incomplete_default_project");
    write_call_site_project(
        &project,
        r#"
            #[anneal(spec(ensures(name = "returns_true", prop = ret)))]
            pub fn contradiction() -> bool { false }
        "#,
    );

    let bin_path = cargo_anneal_bin_path();
    ensure_test_toolchain(&bin_path);

    let lean = temp_dir.path().join("rejected_lean");
    let output =
        generate_project(&bin_path, &project, &temp_dir.path().join("rejected_llbc"), &lean);
    assert!(
        !output.status.success(),
        "incomplete default proof unexpectedly passed\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let soundness = generated_soundness(&lean);
    assert!(soundness.contains("all_goals sorry"));
    assert!(soundness.contains("anneal__contradiction.returns_true"));
    assert!(soundness.contains("theorem contradiction_soundness"));
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("declaration uses `sorry`"),
        "unexpected stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let accepted = generate_project_with_allow_sorry(
        &bin_path,
        &project,
        &temp_dir.path().join("accepted_llbc"),
        &temp_dir.path().join("accepted_lean"),
    );
    assert!(
        accepted.status.success(),
        "--allow-sorry did not permit an incomplete generated proof\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&accepted.stdout),
        String::from_utf8_lossy(&accepted.stderr)
    );
}

#[cfg(feature = "exocrate_tests")]
#[test]
fn test_generate_rejects_false_postcondition() {
    let temp_dir = tempfile::tempdir().unwrap();
    let project = temp_dir.path().join("false_postcondition_project");
    write_call_site_project(
        &project,
        r#"
            #[anneal(spec(ensures(name = "wrong", prop = ret != value)))]
            pub fn identity(value: u32) -> u32 { value }
        "#,
    );

    let bin_path = cargo_anneal_bin_path();
    ensure_test_toolchain(&bin_path);

    let lean = temp_dir.path().join("false_postcondition_lean");
    let output = generate_project(
        &bin_path,
        &project,
        &temp_dir.path().join("false_postcondition_llbc"),
        &lean,
    );
    assert!(
        !output.status.success(),
        "false postcondition unexpectedly passed\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let soundness = generated_soundness(&lean);
    assert!(soundness.contains("anneal__identity.wrong"));
    assert!(soundness.contains("theorem identity_soundness"));
}

#[cfg(feature = "exocrate_tests")]
#[test]
fn test_generate_requires_allow_sorry_for_admitted_lean_proofs() {
    let temp_dir = tempfile::tempdir().unwrap();
    let project = temp_dir.path().join("sorry_project");
    write_call_site_project(
        &project,
        r#"
            #[anneal(proof(lean::tactic = "exact sorryAx _ true"))]
            pub fn identity(value: u32) -> u32 { value }
        "#,
    );

    let bin_path = cargo_anneal_bin_path();
    ensure_test_toolchain(&bin_path);

    let rejected = generate_project(
        &bin_path,
        &project,
        &temp_dir.path().join("rejected_llbc"),
        &temp_dir.path().join("rejected_lean"),
    );
    assert!(!rejected.status.success(), "admitted proof passed without --allow-sorry");
    assert!(
        String::from_utf8_lossy(&rejected.stderr).contains("declaration uses `sorry`"),
        "unexpected stderr: {}",
        String::from_utf8_lossy(&rejected.stderr)
    );

    let accepted = generate_project_with_allow_sorry(
        &bin_path,
        &project,
        &temp_dir.path().join("accepted_llbc"),
        &temp_dir.path().join("accepted_lean"),
    );
    assert!(
        accepted.status.success(),
        "--allow-sorry did not permit the admitted proof\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&accepted.stdout),
        String::from_utf8_lossy(&accepted.stderr)
    );
}

#[cfg(feature = "exocrate_tests")]
#[test]
fn test_generate_rejects_axioms_added_by_proof_metaprogramming() {
    let temp_dir = tempfile::tempdir().unwrap();
    let project = temp_dir.path().join("metaprogrammed_axiom_project");
    write_call_site_project(
        &project,
        r#"
            #[anneal(
                spec(ensures(name = "impossible", prop = ret != value)),
                proof(lean::tactic = "run_tac
  Lean.addDecl (.axiomDecl {
    «name» := `call_site_demo.identity_soundness.forged
    levelParams := []
    type := .const ``False []
    isUnsafe := false
  })
exact False.elim call_site_demo.identity_soundness.forged"),
            )]
            pub fn identity(value: u32) -> u32 { value }
        "#,
    );

    let bin_path = cargo_anneal_bin_path();
    ensure_test_toolchain(&bin_path);

    // `--allow-sorry` deliberately relaxes admissions, but must never permit a
    // proof elaborator to extend the environment with a fresh axiom.
    let output = generate_project_with_allow_sorry(
        &bin_path,
        &project,
        &temp_dir.path().join("llbc"),
        &temp_dir.path().join("lean"),
    );
    assert!(
        !output.status.success(),
        "metaprogrammed axiom unexpectedly passed\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Lean axiom audit failed"), "unexpected stderr: {stderr}");
    assert!(
        stderr.contains("rejected axiom declaration")
            && stderr.contains("identity_soundness.forged"),
        "unexpected stderr: {stderr}"
    );
}

#[cfg(feature = "exocrate_tests")]
#[test]
fn test_generate_rejects_annotations_hidden_from_original_source_scan() {
    let temp_dir = tempfile::tempdir().unwrap();
    let project = temp_dir.path().join("hidden_annotation_project");
    write_call_site_project(
        &project,
        r#"
            use anneal_macros::anneal as verify;

            #[verify(spec(ensures(prop = ret)))]
            pub fn aliased_contradiction() -> bool { false }

            macro_rules! generated_spec {
                () => {
                    #[anneal(spec(ensures(prop = ret)))]
                    pub fn generated_contradiction() -> bool { false }
                };
            }
            generated_spec!();
        "#,
    );

    let bin_path = cargo_anneal_bin_path();
    ensure_test_toolchain(&bin_path);
    let output = generate_project(
        &bin_path,
        &project,
        &temp_dir.path().join("llbc"),
        &temp_dir.path().join("lean"),
    );
    assert!(
        !output.status.success(),
        "source-hidden Anneal annotations unexpectedly passed\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("original-source scan disagrees with rustc macro expansion"));
    assert!(stderr.contains("aliased_contradiction"));
    assert!(stderr.contains("generated_contradiction"));
}

#[cfg(feature = "exocrate_tests")]
#[test]
fn test_generate_v2_examples_with_example_flag_in_dev_mode() {
    let temp_dir = tempfile::tempdir().unwrap();
    let examples_manifest =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("examples").join("Cargo.toml");
    let bin_path = cargo_anneal_bin_path();
    ensure_test_toolchain(&bin_path);

    for example in ["simple", "postcondition", "function_invariant", "lean_escape_hatch"] {
        let output_dir = temp_dir.path().join(example).join("llbc");
        let lean_output_dir = temp_dir.path().join(example).join("lean");
        let output =
            generate_example(&bin_path, &examples_manifest, example, &output_dir, &lean_output_dir);
        assert!(
            output.status.success(),
            "{example} example failed\nstdout: {}\nstderr: {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        let soundness = generated_soundness(&lean_output_dir);
        assert!(soundness.contains("_soundness"));
        if example == "lean_escape_hatch" {
            assert!(soundness.contains(
                "def identity_post_lean_identity (value : Std.U32) (result : Std.U32) : Prop"
            ));
            assert!(soundness.contains("result.val = value.val"));
        }

        if example == "simple" {
            let rerun = generate_example(
                &bin_path,
                &examples_manifest,
                example,
                &output_dir,
                &lean_output_dir,
            );
            assert!(
                rerun.status.success(),
                "{example} example rerun failed\nstdout: {}\nstderr: {}",
                String::from_utf8_lossy(&rerun.stdout),
                String::from_utf8_lossy(&rerun.stderr)
            );
        }
    }
}

#[cfg(feature = "exocrate_tests")]
fn write_call_site_project(project_dir: &std::path::Path, lib_rs: &str) {
    std::fs::create_dir_all(project_dir.join("src")).unwrap();
    let macro_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("anneal-macros");
    std::fs::write(
        project_dir.join("Cargo.toml"),
        format!(
            r#"
            [package]
            name = "call_site_demo"
            version = "0.1.0"
            edition = "2021"

            [dependencies]
            anneal-macros = {{ path = "{}" }}

            [lib]
            path = "src/lib.rs"
        "#,
            macro_path.display()
        ),
    )
    .unwrap();
    std::fs::write(project_dir.join("src/lib.rs"), format!("use anneal_macros::anneal;\n{lib_rs}"))
        .unwrap();
}

#[cfg(feature = "exocrate_tests")]
fn generate_project(
    bin_path: &std::path::Path,
    project_dir: &std::path::Path,
    output_dir: &std::path::Path,
    lean_output_dir: &std::path::Path,
) -> std::process::Output {
    let mut cmd = cargo_anneal_command(bin_path);
    if let Some(path) = std::env::var_os("PATH") {
        cmd.env("PATH", path);
    }
    cmd.arg("generate")
        .arg("--manifest-path")
        .arg(project_dir.join("Cargo.toml"))
        .arg("--lib")
        .arg("--output-dir")
        .arg(output_dir)
        .arg("--lean-output-dir")
        .arg(lean_output_dir)
        .arg("--no-progress");
    cmd.output().expect("failed to execute cargo-anneal")
}

#[cfg(feature = "exocrate_tests")]
fn generate_project_with_allow_sorry(
    bin_path: &std::path::Path,
    project_dir: &std::path::Path,
    output_dir: &std::path::Path,
    lean_output_dir: &std::path::Path,
) -> std::process::Output {
    let mut cmd = cargo_anneal_command(bin_path);
    if let Some(path) = std::env::var_os("PATH") {
        cmd.env("PATH", path);
    }
    cmd.arg("generate")
        .arg("--manifest-path")
        .arg(project_dir.join("Cargo.toml"))
        .arg("--lib")
        .arg("--output-dir")
        .arg(output_dir)
        .arg("--lean-output-dir")
        .arg(lean_output_dir)
        .arg("--allow-sorry")
        .arg("--no-progress");
    cmd.output().expect("failed to execute cargo-anneal")
}

#[cfg(feature = "exocrate_tests")]
fn generate_example(
    bin_path: &std::path::Path,
    examples_manifest: &std::path::Path,
    example: &str,
    output_dir: &std::path::Path,
    lean_output_dir: &std::path::Path,
) -> std::process::Output {
    let mut cmd = cargo_anneal_command(bin_path);
    if let Some(path) = std::env::var_os("PATH") {
        cmd.env("PATH", path);
    }
    cmd.arg("generate")
        .arg("--manifest-path")
        .arg(examples_manifest)
        .arg("--example")
        .arg(example)
        .arg("--output-dir")
        .arg(output_dir)
        .arg("--lean-output-dir")
        .arg(lean_output_dir)
        .arg("--no-progress");
    cmd.output().expect("failed to execute cargo-anneal")
}

#[cfg(feature = "exocrate_tests")]
fn generated_soundness(lean_output_dir: &std::path::Path) -> String {
    for entry in std::fs::read_dir(lean_output_dir).unwrap() {
        let path = entry.unwrap().path().join("Soundness.lean");
        if path.is_file() {
            return std::fs::read_to_string(path).unwrap();
        }
    }
    panic!("No generated Soundness.lean found in {:?}", lean_output_dir);
}
