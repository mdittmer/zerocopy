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
    cmd.env_clear()
        .env("__ANNEAL_LOCAL_DEV", "1")
        .env("CARGO_MANIFEST_DIR", env!("CARGO_MANIFEST_DIR"));
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
fn only_llbc_file(output_dir: &std::path::Path) -> std::path::PathBuf {
    let llbc_files = std::fs::read_dir(output_dir)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "llbc"))
        .collect::<Vec<_>>();

    assert_eq!(llbc_files.len(), 1, "expected one LLBC file in {output_dir:?}");
    llbc_files.into_iter().next().unwrap()
}

#[cfg(feature = "exocrate_tests")]
fn find_fun<'a>(
    crate_data: &'a charon_lib::export::CrateData,
    name: &str,
) -> &'a charon_lib::ast::FunDecl {
    crate_data
        .translated
        .fun_decls
        .iter()
        .find(|fun| {
            fun.item_meta.name.name.iter().any(
                |elem| matches!(elem, charon_lib::ast::PathElem::Ident(ident, _) if ident == name),
            )
        })
        .unwrap_or_else(|| panic!("could not find function {name:?} in LLBC"))
}

#[cfg(feature = "exocrate_tests")]
fn anneal_metadata_doc(fun: &charon_lib::ast::FunDecl) -> Option<&str> {
    fun.item_meta.attr_info.attributes.iter().find_map(|attr| match attr {
        charon_lib::ast::Attribute::DocComment(comment) => {
            comment.trim_start().strip_prefix("anneal:v2:")
        }
        _ => None,
    })
}

#[cfg(feature = "exocrate_tests")]
#[test]
fn test_expand_subcommand_simple() {
    let temp_dir = tempfile::tempdir().unwrap();
    let project_dir = temp_dir.path().join("project");
    let output_dir = temp_dir.path().join("llbc_out");
    std::fs::create_dir_all(project_dir.join("examples")).unwrap();
    std::fs::write(
        project_dir.join("Cargo.toml"),
        r#"
            [package]
            name = "test_proj"
            version = "0.1.0"
            edition = "2021"

            [[example]]
            name = "simple"
            path = "examples/simple.rs"
        "#,
    )
    .unwrap();
    std::fs::write(
        project_dir.join("examples").join("simple.rs"),
        r#"
            pub fn add(left: usize, right: usize) -> usize {
                left + right
            }

            fn main() {
                println!("Hello, world! {}", add(1, 2));
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
    cmd.arg("expand")
        .arg("--manifest-path")
        .arg(project_dir.join("Cargo.toml"))
        .arg("--example")
        .arg("simple")
        .arg("--output-dir")
        .arg(&output_dir);
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
}

#[cfg(feature = "exocrate_tests")]
#[test]
fn test_expand_preserves_attribute_metadata_and_opacity() {
    let temp_dir = tempfile::tempdir().unwrap();
    let project_dir = temp_dir.path().join("project");
    let output_dir = temp_dir.path().join("llbc_out");
    let annotations_dir =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("anneal-annotations");
    std::fs::create_dir_all(project_dir.join("src")).unwrap();
    std::fs::write(
        project_dir.join("Cargo.toml"),
        format!(
            r#"
            [package]
            name = "test_proj"
            version = "0.1.0"
            edition = "2024"

            [dependencies]
            anneal = {{ package = "anneal-annotations", path = "{}" }}
        "#,
            annotations_dir.display()
        ),
    )
    .unwrap();
    std::fs::write(
        project_dir.join("src").join("lib.rs"),
        r#"
            #![register_tool(aeneas)]

            #[anneal::unsafe_axiom(
                requires(non_null = "ptr != 0"),
                ensures(value = "ret = 7")
            )]
            pub unsafe fn read(ptr: *const u32) -> u32 {
                unsafe { *ptr }
            }

            #[anneal::spec(
                ensures(value = "ret = 7"),
                proof(value = "simp [seven]")
            )]
            pub fn seven() -> u32 {
                7
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
    cmd.arg("expand")
        .arg("--manifest-path")
        .arg(project_dir.join("Cargo.toml"))
        .arg("--lib")
        .arg("--output-dir")
        .arg(&output_dir)
        .arg("--no-progress");

    let output = cmd.output().expect("failed to execute cargo-anneal");

    println!("stdout: {}", String::from_utf8_lossy(&output.stdout));
    println!("stderr: {}", String::from_utf8_lossy(&output.stderr));

    assert!(output.status.success(), "cargo-anneal failed");

    let llbc_path = only_llbc_file(&output_dir);
    let file = std::fs::File::open(llbc_path).unwrap();
    let crate_data: charon_lib::export::CrateData = serde_json::from_reader(file).unwrap();
    let read = find_fun(&crate_data, "read");
    let seven = find_fun(&crate_data, "seven");

    assert!(matches!(read.body, charon_lib::ast::Body::Opaque));
    assert!(matches!(read.item_meta.opacity, charon_lib::ast::ItemOpacity::Opaque));
    assert!(matches!(seven.body, charon_lib::ast::Body::Structured(_)));

    let read_metadata =
        anneal_metadata_doc(read).expect("read should have Anneal metadata doc comment");
    let read_metadata: serde_json::Value = serde_json::from_str(read_metadata).unwrap();
    assert_eq!(read_metadata["kind"], "unsafe_axiom");
    assert_eq!(read_metadata["sections"][0]["kind"], "requires");
    assert_eq!(read_metadata["sections"][0]["name"], "non_null");
    assert_eq!(read_metadata["sections"][0]["lean"], "ptr != 0");
    assert_eq!(read_metadata["sections"][1]["kind"], "ensures");
    assert_eq!(read_metadata["sections"][1]["lean"], "ret = 7");

    let seven_metadata =
        anneal_metadata_doc(seven).expect("seven should have Anneal metadata doc comment");
    let seven_metadata: serde_json::Value = serde_json::from_str(seven_metadata).unwrap();
    assert_eq!(seven_metadata["kind"], "spec");
    assert_eq!(seven_metadata["sections"][0]["kind"], "ensures");
    assert_eq!(seven_metadata["sections"][0]["lean"], "ret = 7");
    assert_eq!(seven_metadata["sections"][1]["kind"], "proof");
    assert_eq!(seven_metadata["sections"][1]["lean"], "simp [seven]");
}
