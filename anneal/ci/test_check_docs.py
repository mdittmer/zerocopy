# Copyright 2026 The Fuchsia Authors
#
# Licensed under a BSD-style license <LICENSE-BSD>, Apache License, Version 2.0
# <LICENSE-APACHE or https://www.apache.org/licenses/LICENSE-2.0>, or the MIT
# license <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your option.
# This file may not be copied, modified, or distributed except according to
# those terms.

"""Unit and negative-oracle tests for `ci/check_docs.py`."""

import hashlib
import io
import json
import os
from pathlib import Path
import stat
import sys
import tarfile
import tempfile
import unittest
from unittest import mock

import check_docs
from workflow_yaml import load_workflow_yaml


ANNEAL_ROOT = Path(__file__).resolve().parent.parent


def _make_minimal_anneal_root(root: Path) -> Path:
    """Create a valid minimal `anneal/` tree for isolated oracle tests."""
    root.mkdir(parents=True, exist_ok=True)
    (root / "PRINCIPLES.md").write_text(
        "# Principles\n\nSee [Threat\nModel](THREAT_MODEL.md).\n",
        encoding="utf-8",
    )
    (root / "THREAT_MODEL.md").write_text(
        "# Threat Model\n\nSee [Trust Model](docs/trust-model.md).\n",
        encoding="utf-8",
    )
    (root / "META_PRINCIPLES.md").write_text(
        "# Meta-Principles\n\nSee [Principles](PRINCIPLES.md).\n",
        encoding="utf-8",
    )
    (root / "book/src/reference").mkdir(parents=True)
    (root / "book/examples/cli/help").mkdir(parents=True)
    (root / "book/book.toml").write_text(
        '[book]\ntitle = "Anneal"\n'
        '[output.html]\nsite-url = "/zerocopy/anneal/"\n'
        'git-repository-url = "https://github.com/google/zerocopy/tree/main/anneal"\n'
        'default-theme = "light"\n'
        'preferred-dark-theme = "navy"\n',
        encoding="utf-8",
    )
    (root / "book/src/SUMMARY.md").write_text(
        "# Summary\n\n"
        "- [Introduction](index.md)\n"
        "- [CLI](reference/cli.md)\n"
        "- [Principles](foundations/principles.md)\n"
        "- [Threat Model](foundations/threat-model.md)\n"
        "- [Meta-Principles](foundations/meta-principles.md)\n",
        encoding="utf-8",
    )
    (root / "book/src/index.md").write_text(
        "# Anneal\n\nWelcome.\n", encoding="utf-8"
    )
    (root / "book/src/reference/cli.md").write_text(
        "# CLI\n\n"
        "```console\n"
        "{{#include ../../examples/cli/help/transcript.txt}}\n"
        "```\n",
        encoding="utf-8",
    )
    (root / "book/examples/cli/help/case.json").write_text(
        json.dumps(
            {
                "kind": "cli",
                "display_command": "$ cargo anneal --help",
                "argv": ["anneal", "--help"],
                "expect_exit": 0,
                "transcript": "transcript.txt",
            },
            indent=2,
        )
        + "\n",
        encoding="utf-8",
    )
    (root / "book/examples/cli/help/transcript.txt").write_text(
        "$ cargo anneal --help\nUsage: cargo anneal\n",
        encoding="utf-8",
    )
    return root


class CheckSourcesOracleTests(unittest.TestCase):
    def test_valid_minimal_tree_passes(self):
        with tempfile.TemporaryDirectory(prefix="anneal-docs-test-") as tmp:
            root = _make_minimal_anneal_root(Path(tmp))
            self.assertEqual(check_docs.check_sources(root), [])

    def test_raw_inline_fences_and_commonmark_desyncs_fail(self):
        cases = [
            ("rust", "```rust\nfn main() {}\n```"),
            ("lean", "```lean\ndef x : Nat := 0\n```"),
            ("json", '```json\n{"status": "ok"}\n```'),
            ("sh", "```sh\necho hi\n```"),
            ("reordered_tags", "```no_run,rust\nfn main() {}\n```"),
            ("html_entity_info", "```&#114;ust\nfn main() {}\n```"),
            ("backslash_escaped_comma", "```rust\\,no_run\nfn main() {}\n```"),
            ("blockquote_fence", "> ```rust\n> fn main() {}\n> ```"),
            ("list_item_fence", "- ```rust\n  fn main() {}\n  ```"),
            ("nested_blockquote_in_list", "- > ```rust\n  > fn main() {}\n  > ```"),
            (
                "four_space_indented_desync",
                "    ````\n```rust\nfn unverified() {}\n```\n<script>alert(1)</script>\n    ````",
            ),
            (
                "list_exit_breakout",
                "- ```text\n```rust\nfn unverified() {}\n```\n</main><script>alert(1)</script><main>\n  ```",
            ),
        ]
        for label, snippet in cases:
            with self.subTest(case=label):
                with tempfile.TemporaryDirectory(prefix="anneal-docs-test-") as tmp:
                    root = _make_minimal_anneal_root(Path(tmp))
                    (root / "book/src/index.md").write_text(
                        f"# Intro\n\n{snippet}\n", encoding="utf-8"
                    )
                    errors = check_docs.check_sources(root)
                    self.assertTrue(
                        any(
                            "code fence" in e or "unclosed fenced" in e
                            for e in errors
                        ),
                        errors,
                    )
                    if label in ("four_space_indented_desync", "list_exit_breakout"):
                        self.assertTrue(
                            any("raw HTML tags are forbidden" in e for e in errors),
                            errors,
                        )

    def test_conceptual_fence_forbidden_in_strict_chapters(self):
        with tempfile.TemporaryDirectory(prefix="anneal-docs-test-") as tmp:
            root = _make_minimal_anneal_root(Path(tmp))
            (root / "book/src/reference/cli.md").write_text(
                "# CLI\n\n"
                "```console\n"
                "{{#include ../../examples/cli/help/transcript.txt}}\n"
                "```\n\n"
                "```rust,conceptual\nfn sketch() {}\n```\n",
                encoding="utf-8",
            )
            errors = check_docs.check_sources(root)
            self.assertTrue(
                any("`conceptual` code fence is forbidden" in e for e in errors),
                errors,
            )

    def test_disallowed_mdbook_directives_and_unfenced_includes_fail(self):
        bad_directives = [
            ("playground", "{{#playground ../examples/cli/help/transcript.txt}}", "disallowed or malformed"),
            ("trailing_space", "{{#include ../examples/cli/help/transcript.txt }}", "disallowed or malformed"),
            ("extra_args", "{{#include ../examples/cli/help/transcript.txt 1:2}}", "disallowed or malformed"),
            ("quoted_path", '{{#include "../examples/cli/help/transcript.txt"}}', "disallowed or malformed"),
            ("unclosed", "{{#include ../examples/cli/help/transcript.txt", "disallowed or malformed"),
            ("unfenced_prose_include", "{{#include ../examples/cli/help/transcript.txt}}", "only permitted inside a checked code fence"),
        ]
        for label, directive, expected in bad_directives:
            with self.subTest(case=label):
                with tempfile.TemporaryDirectory(prefix="anneal-docs-test-") as tmp:
                    root = _make_minimal_anneal_root(Path(tmp))
                    (root / "book/src/index.md").write_text(
                        f"# Intro\n\n{directive}\n", encoding="utf-8"
                    )
                    errors = check_docs.check_sources(root)
                    self.assertTrue(any(expected in e for e in errors), errors)

    def test_unverified_file_bad_ranges_and_anchors_via_check_sources(self):
        with tempfile.TemporaryDirectory(prefix="anneal-docs-test-") as tmp:
            root = _make_minimal_anneal_root(Path(tmp))
            (root / "book/examples/cli/help/unverified.txt").write_text(
                "unverified\n", encoding="utf-8"
            )
            (root / "book/examples/cli/help/transcript.txt").write_text(
                "$ cargo anneal --help\n"
                "// ANCHOR: empty\n// ANCHOR_END: empty\n"
                "// ANCHOR: dup\nline\n// ANCHOR_END: dup\n"
                "// ANCHOR: dup\nline2\n// ANCHOR_END: dup\n"
                "// ANCHOR: unclosed\n",
                encoding="utf-8",
            )
            (root / "book/src/index.md").write_text(
                "# Intro\n\n"
                "```console\n"
                "{{#include ../examples/cli/help/unverified.txt}}\n"
                "```\n\n"
                "```console\n"
                "{{#include ../examples/cli/help/transcript.txt:foo:bar}}\n"
                "```\n\n"
                "```console\n"
                "{{#include ../examples/cli/help/transcript.txt:1_0:2}}\n"
                "```\n\n"
                "```console\n"
                "{{#include ../examples/cli/help/transcript.txt:+1:2}}\n"
                "```\n\n"
                "```console\n"
                "{{#include ../examples/cli/help/transcript.txt:empty}}\n"
                "```\n\n"
                "```console\n"
                "{{#include ../examples/cli/help/transcript.txt:missing}}\n"
                "```\n",
                encoding="utf-8",
            )
            errors = check_docs.check_sources(root)
            self.assertTrue(
                any("not a registered verified artifact" in e for e in errors),
                errors,
            )
            self.assertEqual(
                sum("non-integer line range" in e for e in errors), 3, errors
            )
            self.assertTrue(any("anchor `empty`" in e for e in errors), errors)
            self.assertTrue(
                any("missing anchor `missing`" in e for e in errors), errors
            )
            self.assertTrue(
                any("duplicate ANCHOR start `dup`" in e for e in errors), errors
            )
            self.assertTrue(
                any("unclosed ANCHOR `unclosed`" in e for e in errors), errors
            )

    def test_hidden_rustdoc_unsafe_and_inner_doc_anneal_spec_fail(self):
        with tempfile.TemporaryDirectory(prefix="anneal-docs-test-") as tmp:
            root = _make_minimal_anneal_root(Path(tmp))
            rs_path = root / "book/examples/cli/help/demo.rs"
            rs_path.write_text(
                "//! ```anneal\n"
                "// ANCHOR: shown\n"
                "# unsafe fn hidden_by_rustdoc() {}\n"
                "pub fn safe_id(x: usize) -> usize { x }\n"
                "// ANCHOR_END: shown\n"
                "/// anneal:requires x != 0\n"
                "pub unsafe fn hidden_outside(x: usize) -> usize { x }\n",
                encoding="utf-8",
            )
            errors: list[str] = []
            check_docs._validate_rust_anchor_coverage(
                rs_path, [(3, 4, "include")], errors
            )
            self.assertTrue(any("```anneal" in e for e in errors), errors)
            self.assertTrue(any("hidden_by_rustdoc" in e for e in errors), errors)
            self.assertTrue(any("hidden_outside" in e for e in errors), errors)

    def test_unreferenced_example_case_fails_even_if_case_json_included(self):
        with tempfile.TemporaryDirectory(prefix="anneal-docs-test-") as tmp:
            root = _make_minimal_anneal_root(Path(tmp))
            orphan = root / "book/examples/cli/orphan"
            orphan.mkdir(parents=True)
            (orphan / "case.json").write_text(
                json.dumps(
                    {
                        "kind": "cli",
                        "display_command": "$ cargo anneal --version",
                        "argv": ["anneal", "--version"],
                        "expect_exit": 0,
                        "transcript": "transcript.txt",
                    }
                ),
                encoding="utf-8",
            )
            (orphan / "transcript.txt").write_text(
                "$ cargo anneal --version\ncargo-anneal 0.1.0\n",
                encoding="utf-8",
            )
            (root / "book/src/index.md").write_text(
                "# Intro\n\n```console\n{{#include ../examples/cli/orphan/case.json}}\n```\n",
                encoding="utf-8",
            )
            errors = check_docs.check_sources(root)
            self.assertTrue(
                any("not a registered verified artifact" in e for e in errors),
                errors,
            )
            self.assertTrue(
                any("verified artifact is not included" in e for e in errors),
                errors,
            )

    def test_exact_case_allowlist_and_raw_html_tags_fail(self):
        for rel_path in (
            "Cargo.toml",
            "helper.py",
            "Proof.lean",
            "theme/index.hbs",
            "src/x.html",
            "src/pic.svg",
            "src/page.xhtml",
            "src/evil.MD",
            "examples/cli/help/Cargo.toml",
            "examples/cli/help/cargo.lock",
            "examples/cli/help/demo.RS",
        ):
            with self.subTest(path=rel_path):
                with tempfile.TemporaryDirectory(prefix="anneal-docs-test-") as tmp:
                    root = _make_minimal_anneal_root(Path(tmp))
                    target = root / "book" / rel_path
                    target.parent.mkdir(parents=True, exist_ok=True)
                    target.write_text("", encoding="utf-8")
                    errors = check_docs.check_sources(root)
                    self.assertTrue(
                        len(errors) > 0, f"expected rejection for {rel_path}"
                    )

        for label, bad_md in (
            ("script", "<script>alert(1)</script>"),
            ("img_multiline", '<img src="x" onerror\n="alert(1)">'),
            ("main_breakout", "</main><p>x</p>"),
            ("base_tag", '<base href="https://attacker.example/">'),
            ("entity_js_link", '<a href="&#106;avascript:alert(1)">click</a>'),
            ("md_js_link", "[click](javascript:alert(1))"),
            ("multiline_backtick_breakout", "`foo\n<script>alert(1)</script>\nbar`"),
        ):
            with self.subTest(html=label):
                with tempfile.TemporaryDirectory(prefix="anneal-docs-test-") as tmp:
                    root = _make_minimal_anneal_root(Path(tmp))
                    (root / "book/src/index.md").write_text(
                        f"# Intro\n\n{bad_md}\n", encoding="utf-8"
                    )
                    errors = check_docs.check_sources(root)
                    self.assertTrue(
                        any(
                            "raw HTML tags are forbidden" in e
                            or "disallowed script/data URL scheme" in e
                            for e in errors
                        ),
                        errors,
                    )

    def test_unmapped_wrapped_governing_doc_link_fails(self):
        with tempfile.TemporaryDirectory(prefix="anneal-docs-test-") as tmp:
            root = _make_minimal_anneal_root(Path(tmp))
            (root / "PRINCIPLES.md").write_text(
                "# Principles\n\nSee [Wrapped\nLink](docs/unmapped.md).\n",
                encoding="utf-8",
            )
            errors = check_docs.check_sources(root)
            self.assertTrue(
                any("unmapped relative link `docs/unmapped.md`" in e for e in errors),
                errors,
            )

    def test_duplicate_checked_in_governing_doc_fails(self):
        with tempfile.TemporaryDirectory(prefix="anneal-docs-test-") as tmp:
            root = _make_minimal_anneal_root(Path(tmp))
            (root / "book/src/foundations").mkdir(parents=True)
            (root / "book/src/foundations/principles.md").write_text(
                "# Duplicate\n", encoding="utf-8"
            )
            errors = check_docs.check_sources(root)
            self.assertTrue(
                any("do not check in a duplicate copy" in e for e in errors),
                errors,
            )

    def test_book_toml_disallowed_sections_missing_site_url_and_themes_fail(self):
        bad_configs = [
            (
                "preprocessor",
                '[book]\ntitle = "Anneal"\n[output.html]\nsite-url = "/zerocopy/anneal/"\n[preprocessor.evil]\ncommand = "true"\n',
                "disallowed top-level sections",
            ),
            (
                "custom_output",
                '[book]\ntitle = "Anneal"\n[output.html]\nsite-url = "/zerocopy/anneal/"\n[output.evil]\ncommand = "true"\n',
                "disallowed `[output]` sections",
            ),
            (
                "additional_js",
                '[book]\ntitle = "Anneal"\n[output.html]\nsite-url = "/zerocopy/anneal/"\nadditional-js = ["x.js"]\n',
                "disallowed `[output.html]` keys",
            ),
            (
                "missing_output",
                '[book]\ntitle = "Anneal"\n',
                "missing `[output]` table",
            ),
            (
                "missing_site_url",
                '[book]\ntitle = "Anneal"\n[output.html]\ndefault-theme = "light"\n',
                "`[output.html].site-url` must be",
            ),
            (
                "bad_site_url",
                '[book]\ntitle = "Anneal"\n[output.html]\nsite-url = "https://attacker.example/"\n',
                "`[output.html].site-url` must be",
            ),
            (
                "bad_repo_url",
                '[book]\ntitle = "Anneal"\n[output.html]\nsite-url = "/zerocopy/anneal/"\ngit-repository-url = "javascript:alert(1)"\n',
                "`[output.html].git-repository-url` must start with",
            ),
            (
                "custom_theme_name",
                '[book]\ntitle = "Anneal"\n[output.html]\nsite-url = "/zerocopy/anneal/"\ndefault-theme = "custom-injected"\n',
                "must be a built-in mdBook theme",
            ),
        ]
        for label, toml_text, expected in bad_configs:
            with self.subTest(case=label):
                with tempfile.TemporaryDirectory(prefix="anneal-docs-test-") as tmp:
                    root = _make_minimal_anneal_root(Path(tmp))
                    (root / "book/book.toml").write_text(
                        toml_text, encoding="utf-8"
                    )
                    errors = check_docs.check_sources(root)
                    self.assertTrue(any(expected in e for e in errors), errors)

    def test_cli_argv_allowlist_traversal_and_case_schema_fail_closed(self):
        bad_cases = [
            (
                "bare_setup_network",
                {
                    "kind": "cli",
                    "display_command": "$ cargo anneal setup",
                    "argv": ["anneal", "setup"],
                    "expect_exit": 0,
                    "transcript": "transcript.txt",
                },
                "closed offline CLI allowlist",
            ),
            (
                "future_verify_unsandboxed",
                {
                    "kind": "cli",
                    "display_command": "$ cargo anneal verify",
                    "argv": ["anneal", "verify"],
                    "expect_exit": 0,
                    "transcript": "transcript.txt",
                },
                "closed offline CLI allowlist",
            ),
            (
                "absolute_arg",
                {
                    "kind": "cli",
                    "display_command": "$ cargo anneal setup --local-archive /etc/passwd",
                    "argv": ["anneal", "setup", "--local-archive", "/etc/passwd"],
                    "expect_exit": 1,
                    "transcript": "transcript.txt",
                },
                "absolute or parent-traversal paths",
            ),
            (
                "equals_absolute",
                {
                    "kind": "cli",
                    "display_command": "$ cargo anneal setup --local-archive=/etc/passwd",
                    "argv": ["anneal", "setup", "--local-archive=/etc/passwd"],
                    "expect_exit": 1,
                    "transcript": "transcript.txt",
                },
                "absolute or parent-traversal paths",
            ),
            (
                "equals_parent",
                {
                    "kind": "cli",
                    "display_command": "$ cargo anneal setup --local-archive=../secret",
                    "argv": ["anneal", "setup", "--local-archive=../secret"],
                    "expect_exit": 1,
                    "transcript": "transcript.txt",
                },
                "absolute or parent-traversal paths",
            ),
            (
                "display_argv_mismatch",
                {
                    "kind": "cli",
                    "display_command": "$ cargo anneal --old-flag",
                    "argv": ["anneal", "--help"],
                    "expect_exit": 0,
                    "transcript": "transcript.txt",
                },
                "does not match `argv`",
            ),
            (
                "cargo_missing_anneal_subcommand",
                {
                    "kind": "cli",
                    "display_command": "$ cargo setup --help",
                    "argv": ["setup", "--help"],
                    "expect_exit": 0,
                    "transcript": "transcript.txt",
                },
                "does not match `argv`",
            ),
            (
                "bool_exit_code",
                {
                    "kind": "cli",
                    "display_command": "$ cargo anneal --help",
                    "argv": ["anneal", "--help"],
                    "expect_exit": True,
                    "transcript": "transcript.txt",
                },
                "`expect_exit` must be an integer",
            ),
            (
                "bare_setup_clear_old_network",
                {
                    "kind": "cli",
                    "display_command": "$ cargo anneal setup --clear-old",
                    "argv": ["anneal", "setup", "--clear-old"],
                    "expect_exit": 0,
                    "fresh_cache_env": True,
                    "transcript": "transcript.txt",
                },
                "closed offline CLI allowlist",
            ),
            (
                "init_runtime_writes",
                {
                    "kind": "cli",
                    "display_command": "$ cargo anneal init",
                    "argv": ["anneal", "init"],
                    "expect_exit": 0,
                    "transcript": "transcript.txt",
                },
                "closed offline CLI allowlist",
            ),
            (
                "init_standalone_runtime_writes",
                {
                    "kind": "cli",
                    "display_command": "$ cargo anneal init --standalone",
                    "argv": ["anneal", "init", "--standalone"],
                    "expect_exit": 0,
                    "transcript": "transcript.txt",
                },
                "closed offline CLI allowlist",
            ),
            (
                "verify_runtime_unsandboxed",
                {
                    "kind": "cli",
                    "display_command": "$ cargo anneal verify --request req.json",
                    "argv": ["anneal", "verify", "--request", "req.json"],
                    "expect_exit": 0,
                    "transcript": "transcript.txt",
                },
                "closed offline CLI allowlist",
            ),
            (
                "verify_rustc_runtime_unsandboxed",
                {
                    "kind": "cli",
                    "display_command": "$ cargo anneal verify rustc lib.rs",
                    "argv": ["anneal", "verify", "rustc", "lib.rs"],
                    "expect_exit": 0,
                    "transcript": "transcript.txt",
                },
                "closed offline CLI allowlist",
            ),
            (
                "dot_setup_file",
                {
                    "kind": "cli",
                    "display_command": "$ cargo anneal --help",
                    "argv": ["anneal", "--help"],
                    "expect_exit": 0,
                    "transcript": "transcript.txt",
                    "setup_files": {"./.": "bad"},
                },
                "invalid `setup_files` relative path",
            ),
        ]
        for label, case_obj, expected in bad_cases:
            with self.subTest(case=label):
                with tempfile.TemporaryDirectory(prefix="anneal-docs-test-") as tmp:
                    root = _make_minimal_anneal_root(Path(tmp))
                    (root / "book/examples/cli/help/case.json").write_text(
                        json.dumps(case_obj), encoding="utf-8"
                    )
                    errors = check_docs.check_sources(root)
                    self.assertTrue(any(expected in e for e in errors), errors)

    def test_cli_argv_allowlist_admits_safe_offline_subcommands(self):
        good_cases = [
            ("init_help", ["anneal", "init", "--help"], "$ cargo anneal init --help", False, {}),
            ("help_init", ["anneal", "help", "init"], "$ cargo anneal help init", False, {}),
            ("skill", ["anneal", "skill"], "$ cargo anneal skill", False, {}),
            ("skill_help", ["anneal", "skill", "--help"], "$ cargo anneal skill --help", False, {}),
            ("skill_example", ["anneal", "skill", "--example"], "$ cargo anneal skill --example", False, {}),
            ("verify_help", ["anneal", "verify", "--help"], "$ cargo anneal verify --help", False, {}),
            ("verify_rustc_help", ["anneal", "verify", "rustc", "--help"], "$ cargo anneal verify rustc --help", False, {}),
            (
                "setup_local_archive_clear_old",
                ["anneal", "setup", "--local-archive", "bad.tar.zst", "--clear-old"],
                "$ cargo anneal setup --local-archive bad.tar.zst --clear-old",
                True,
                {"bad.tar.zst": "not-an-archive\n"},
            ),
        ]
        for label, argv, display, fresh_cache, setup_files in good_cases:
            with self.subTest(case=label):
                with tempfile.TemporaryDirectory(prefix="anneal-docs-test-") as tmp:
                    root = _make_minimal_anneal_root(Path(tmp))
                    case_obj = {
                        "kind": "cli",
                        "display_command": display,
                        "argv": argv,
                        "expect_exit": 0,
                        "transcript": "transcript.txt",
                    }
                    if fresh_cache:
                        case_obj["fresh_cache_env"] = True
                    if setup_files:
                        case_obj["setup_files"] = setup_files
                    (root / "book/examples/cli/help/case.json").write_text(
                        json.dumps(case_obj), encoding="utf-8"
                    )
                    self.assertEqual(check_docs.check_sources(root), [])

    def test_staged_governing_docs_reject_directives_raw_html_stray_fences_and_local_images(
        self,
    ):
        bad_docs = [
            (
                "prose_include_directive",
                "# Principles\n\n{{#include ../../Cargo.toml}}\n",
                "mdBook directive",
            ),
            (
                "fenced_include_directive",
                "# Principles\n\n```text\n{{#include ../../Cargo.toml}}\n```\n",
                "mdBook directive",
            ),
            (
                "raw_script_html",
                "# Principles\n\n<script>alert(1)</script>\n",
                "raw HTML tags are forbidden",
            ),
            (
                "local_image",
                "# Principles\n\n![secret](secret.png)\n",
                "disallowed or local Markdown image target",
            ),
            (
                "stray_fence",
                "# Principles\n\n    ```text\nhi\n    ```\n",
                "nested or indented code fence marker",
            ),
        ]
        for label, body, expected in bad_docs:
            with self.subTest(case=label):
                with tempfile.TemporaryDirectory(prefix="anneal-docs-test-") as tmp:
                    root = _make_minimal_anneal_root(Path(tmp))
                    (root / "PRINCIPLES.md").write_text(body, encoding="utf-8")
                    errors = check_docs.check_sources(root)
                    self.assertTrue(any(expected in e for e in errors), errors)

    def test_book_markdown_rejects_local_images_and_allows_https_or_backticked_images(
        self,
    ):
        with tempfile.TemporaryDirectory(prefix="anneal-docs-test-") as tmp:
            root = _make_minimal_anneal_root(Path(tmp))
            (root / "book/src/index.md").write_text(
                "# Anneal\n\n"
                "Backticked `![diagram](missing.png)` is fine.\n\n"
                "![logo](https://example.com/logo.svg)\n",
                encoding="utf-8",
            )
            self.assertEqual(check_docs.check_sources(root), [])

            (root / "book/src/index.md").write_text(
                "# Anneal\n\n![diagram](missing.png)\n",
                encoding="utf-8",
            )
            errors = check_docs.check_sources(root)
            self.assertTrue(
                any("disallowed or local Markdown image target" in e for e in errors),
                errors,
            )

    def test_collect_verified_snippets_supports_ranges_anchors_rustdoc_and_multi_include(
        self,
    ):
        with tempfile.TemporaryDirectory(prefix="anneal-docs-test-") as tmp:
            root = _make_minimal_anneal_root(Path(tmp))
            (root / "book/examples/cli/help/transcript.txt").write_text(
                "line1\n"
                "// ANCHOR: outer\n"
                "// ANCHOR: inner\n"
                "visible_a\n"
                "# hidden_line\n"
                "## escaped_hash\n"
                "visible_b\n"
                "// ANCHOR_END: inner\n"
                "// ANCHOR_END: outer\n"
                "line_last\n",
                encoding="utf-8",
            )
            (root / "book/src/index.md").write_text(
                "# Intro\n\n"
                "```console\n"
                "{{#include ../examples/cli/help/transcript.txt:1:1}}\n"
                "```\n\n"
                "```console\n"
                "{{#include ../examples/cli/help/transcript.txt:outer}}\n"
                "```\n\n"
                "```console\n"
                "{{#include ../examples/cli/help/transcript.txt:1:1}}\n"
                "{{#include ../examples/cli/help/transcript.txt:10:10}}\n"
                "```\n\n"
                "```console\n"
                "{{#include ../examples/cli/help/transcript.txt:1:1}}\n\n"
                "{{#include ../examples/cli/help/transcript.txt:10:10}}\n"
                "```\n\n"
                "```rust\n"
                "{{#rustdoc_include ../examples/cli/help/transcript.txt:inner}}\n"
                "```\n",
                encoding="utf-8",
            )
            self.assertEqual(check_docs.check_sources(root), [])
            snippets = check_docs._collect_verified_snippet_texts(root)
            self.assertIn("line1", snippets)
            self.assertIn(
                "visible_a\n# hidden_line\n## escaped_hash\nvisible_b",
                snippets,
            )
            self.assertIn("line1\nline_last", snippets)
            self.assertIn("line1\n\nline_last", snippets)
            self.assertIn(
                "visible_a\n# escaped_hash\nvisible_b",
                snippets,
            )


class VerifyExamplesOracleTests(unittest.TestCase):
    def _write_fake_cli(self, path: Path, body: str) -> Path:
        path.write_text(f"#!{sys.executable}\n{body}", encoding="utf-8")
        path.chmod(path.stat().st_mode | stat.S_IXUSR)
        return path

    def test_matching_and_drifted_cli_transcript_and_bless(self):
        with tempfile.TemporaryDirectory(prefix="anneal-docs-cli-") as tmp:
            tmp_path = Path(tmp)
            root = _make_minimal_anneal_root(tmp_path / "anneal")
            fake_bin = self._write_fake_cli(
                tmp_path / "fake-cargo-anneal",
                "import sys\nprint('Usage: cargo anneal')\n",
            )
            self.assertEqual(check_docs.verify_examples(root, fake_bin), [])

            self._write_fake_cli(
                fake_bin,
                "import sys\nprint('Usage: cargo anneal [NEW_FLAG]')\n",
            )
            errors = check_docs.verify_examples(root, fake_bin)
            self.assertTrue(
                any("transcript mismatch" in e for e in errors), errors
            )

            self.assertEqual(
                check_docs.verify_examples(root, fake_bin, bless=True), []
            )
            self.assertEqual(check_docs.verify_examples(root, fake_bin), [])

    def test_wrong_exit_code_fails(self):
        with tempfile.TemporaryDirectory(prefix="anneal-docs-cli-") as tmp:
            tmp_path = Path(tmp)
            root = _make_minimal_anneal_root(tmp_path / "anneal")
            fake_bin = self._write_fake_cli(
                tmp_path / "fake-cargo-anneal",
                "import sys\nsys.exit(2)\n",
            )
            errors = check_docs.verify_examples(root, fake_bin)
            self.assertTrue(
                any("expected exit code 0, got 2" in e for e in errors), errors
            )

    def test_main_runs_source_check_even_for_verify_examples_alone_and_rejects_mdbook_symlink(
        self,
    ):
        with tempfile.TemporaryDirectory(prefix="anneal-docs-cli-") as tmp:
            tmp_path = Path(tmp)
            root = _make_minimal_anneal_root(tmp_path / "anneal")
            (root / "book/book.toml").write_text(
                '[book]\ntitle = "Anneal"\n[preprocessor.evil]\ncommand = "true"\n',
                encoding="utf-8",
            )
            marker = tmp_path / "ran.txt"
            fake_bin = self._write_fake_cli(
                tmp_path / "fake-cargo-anneal",
                f"from pathlib import Path\nPath({str(marker)!r}).write_text('ran')\n",
            )
            rc = check_docs.main(
                [
                    "--anneal-root",
                    str(root),
                    "--verify-examples",
                    "--cargo-anneal",
                    str(fake_bin),
                ]
            )
            self.assertEqual(rc, 1)
            self.assertFalse(
                marker.exists(),
                "cargo-anneal must not execute when check_sources fails",
            )

            valid_root = _make_minimal_anneal_root(tmp_path / "anneal2")
            real_mdbook = self._write_fake_cli(tmp_path / "real-mdbook", "")
            link_mdbook = tmp_path / "link-mdbook"
            link_mdbook.symlink_to(real_mdbook)
            rc2 = check_docs.main(
                [
                    "--anneal-root",
                    str(valid_root),
                    "--build",
                    "--mdbook",
                    str(link_mdbook),
                    "--output",
                    str(tmp_path / "out"),
                ]
            )
            self.assertEqual(rc2, 1)


class EnsureMdbookTests(unittest.TestCase):
    def _build_tar_gz(self, entries: list[tuple[str, bytes]]) -> bytes:
        buf = io.BytesIO()
        with tarfile.open(fileobj=buf, mode="w:gz") as tf:
            for name, payload in entries:
                info = tarfile.TarInfo(name=name)
                info.size = len(payload)
                tf.addfile(info, io.BytesIO(payload))
        return buf.getvalue()

    def test_rejects_tarball_and_binary_sha_mismatch_and_bad_members(self):
        payload = b"fake-mdbook-binary"
        good_tar = self._build_tar_gz([("mdbook", payload)])
        good_tar_sha = hashlib.sha256(good_tar).hexdigest()
        good_bin_sha = hashlib.sha256(payload).hexdigest()

        with tempfile.TemporaryDirectory(prefix="anneal-mdbook-test-") as tmp:
            cache = Path(tmp) / "cache"

            with mock.patch.object(
                check_docs,
                "_download_with_retries",
                side_effect=lambda _u, dest: dest.write_bytes(good_tar),
            ):
                with self.assertRaisesRegex(check_docs.DocError, "tarball SHA-256"):
                    check_docs.ensure_mdbook(
                        cache,
                        expected_tarball_sha256="0" * 64,
                        expected_bin_sha256=good_bin_sha,
                    )

            with mock.patch.object(
                check_docs,
                "_download_with_retries",
                side_effect=lambda _u, dest: dest.write_bytes(good_tar),
            ):
                with self.assertRaisesRegex(check_docs.DocError, "binary SHA-256"):
                    check_docs.ensure_mdbook(
                        cache,
                        expected_tarball_sha256=good_tar_sha,
                        expected_bin_sha256="f" * 64,
                    )

            bad_tar = self._build_tar_gz([("mdbook", payload), ("extra", b"x")])
            bad_tar_sha = hashlib.sha256(bad_tar).hexdigest()
            with mock.patch.object(
                check_docs,
                "_download_with_retries",
                side_effect=lambda _u, dest: dest.write_bytes(bad_tar),
            ):
                with self.assertRaisesRegex(
                    check_docs.DocError, "unexpected mdBook tarball contents"
                ):
                    check_docs.ensure_mdbook(
                        cache,
                        expected_tarball_sha256=bad_tar_sha,
                        expected_bin_sha256=good_bin_sha,
                    )

            with mock.patch.object(
                check_docs,
                "_download_with_retries",
                side_effect=lambda _u, dest: dest.write_bytes(good_tar),
            ):
                bin_path = check_docs.ensure_mdbook(
                    cache,
                    expected_tarball_sha256=good_tar_sha,
                    expected_bin_sha256=good_bin_sha,
                )
                self.assertTrue(bin_path.is_absolute())
                self.assertEqual(bin_path.read_bytes(), payload)

    def test_explicit_relative_mdbook_path_resolves_and_runs_without_path(self):
        with tempfile.TemporaryDirectory(prefix="anneal-mdbook-rel-") as tmp:
            tmp_path = Path(tmp)
            root = _make_minimal_anneal_root(tmp_path / "anneal")
            fake_mdbook = tmp_path / "mdbook"
            fake_mdbook.write_text(
                f"#!{sys.executable}\n"
                "import sys\n"
                "from pathlib import Path\n"
                "dest = Path(sys.argv[sys.argv.index('--dest-dir') + 1])\n"
                "dest.mkdir(parents=True, exist_ok=True)\n"
                "(dest / 'index.html').write_text(\n"
                "    '<html><body><main><h1>Ok</h1></main></body></html>',\n"
                "    encoding='utf-8',\n"
                ")\n",
                encoding="utf-8",
            )
            fake_mdbook.chmod(fake_mdbook.stat().st_mode | stat.S_IXUSR)

            old_cwd = Path.cwd()
            try:
                os.chdir(tmp_path)
                resolved = check_docs.ensure_mdbook(
                    Path("./cache"), explicit_bin=Path("./mdbook")
                )
                self.assertTrue(resolved.is_absolute())
                self.assertEqual(resolved, fake_mdbook.resolve())

                with mock.patch.dict(os.environ, {"PATH": ""}, clear=False):
                    errors = check_docs.build_book(
                        root,
                        tmp_path / "out",
                        mdbook_bin=Path("./mdbook"),
                    )
                self.assertEqual(errors, [])
            finally:
                os.chdir(old_cwd)


class HtmlLinkCheckerTests(unittest.TestCase):
    def test_valid_and_broken_links_xss_duplicate_attrs_main_breakout_and_code_parity(
        self,
    ):
        verified = {"$ cargo anneal --help\nUsage"}
        with tempfile.TemporaryDirectory(prefix="anneal-docs-html-") as tmp:
            html_root = Path(tmp)
            (html_root / "sub").mkdir()
            (html_root / "index.html").write_text(
                '<html><body><main><h1 id="top">Top</h1>'
                '<a href="sub/page.html#sec">ok</a>'
                '<a href="/zerocopy/anneal/sub/page.html#sec">ok-abs</a>'
                '<a href="https://example.com/external">ext</a>'
                '<pre><code class="language-console">$ cargo anneal --help\nUsage</code></pre>'
                "</main></body></html>",
                encoding="utf-8",
            )
            (html_root / "sub/page.html").write_text(
                '<html><body><main><h2 id="sec">Sec</h2>'
                '<a href="../index.html#top">back</a>'
                "</main></body></html>",
                encoding="utf-8",
            )
            self.assertEqual(
                check_docs.check_built_html_links(
                    html_root, verified_snippets=verified
                ),
                [],
            )

            (html_root / "sub/page.html").write_text(
                '<html><body><main><h2 id="sec">Sec</h2>'
                '<a href="../missing.html">broken-file</a>'
                '<a href="../index.html#no-such-id">broken-anchor</a>'
                '<a href="&#106;avascript:alert(1)" href="../index.html">dup-attr</a>'
                '<base href="https://attacker.example/">'
                '<svg></svg>'
                '<script>alert(1)</script>'
                '<img src="x" onerror="alert(1)">'
                '<pre><code class="language-rust">fn unverified() {}</code></pre>'
                "</main></main></body></html>",
                encoding="utf-8",
            )
            (html_root / "getting-started").mkdir()
            (html_root / "getting-started/install.html").write_text(
                "<html><body><main>"
                '<pre><code class="language-rust conceptual">fn sketch() {}</code></pre>'
                "</main></body></html>",
                encoding="utf-8",
            )
            errors = check_docs.check_built_html_links(
                html_root, verified_snippets=verified
            )
            self.assertTrue(any("broken internal link" in e for e in errors), errors)
            self.assertTrue(any("broken anchor `#no-such-id`" in e for e in errors), errors)
            self.assertTrue(any("duplicate attribute `href`" in e for e in errors), errors)
            self.assertTrue(any("disallowed URL scheme `javascript:`" in e for e in errors), errors)
            self.assertTrue(any("disallowed `<base>` element" in e for e in errors), errors)
            self.assertTrue(any("disallowed `<svg>` element" in e for e in errors), errors)
            self.assertTrue(any("disallowed `<script>` element" in e for e in errors), errors)
            self.assertTrue(any("disallowed event-handler attribute `onerror`" in e for e in errors), errors)
            self.assertTrue(any("unmatched `</main>`" in e for e in errors), errors)
            self.assertTrue(
                any("rendered `rust` code block does not match" in e for e in errors),
                errors,
            )
            self.assertTrue(
                any("rendered `conceptual` `rust` code block is forbidden" in e for e in errors),
                errors,
            )

    def test_html_auditor_checks_local_media_src_and_rust_boring_spans(self):
        verified = {"fn ok() {}"}
        boring = set(check_docs.RUST_PLAYGROUND_BOILERPLATE_LINES) | {"let hidden = 1;"}
        with tempfile.TemporaryDirectory(prefix="anneal-docs-html-src-") as tmp:
            html_root = Path(tmp)
            (html_root / "sub").mkdir()
            (html_root / "ok.png").write_bytes(b"png")
            (html_root / "index.html").write_text(
                "<html><body><main>"
                '<img src="ok.png">'
                '<img src="/zerocopy/anneal/ok.png">'
                '<img src="https://example.com/logo.svg">'
                '<pre class="playground"><code class="language-rust">'
                '<span class="boring">#![allow(unused)]\n</span>'
                '<span class="boring">fn main() {\n</span>'
                '<span class="boring">let hidden = 1;\n</span>'
                "fn ok() {}\n"
                '<span class="boring">}</span>'
                "</code></pre>"
                "</main></body></html>",
                encoding="utf-8",
            )
            self.assertEqual(
                check_docs.check_built_html_links(
                    html_root,
                    verified_snippets=verified,
                    verified_boring_lines=boring,
                ),
                [],
            )

            (html_root / "index.html").write_text(
                "<html><body><main>"
                '<img src="missing.png">'
                '<img src="sub">'
                '<img src="../escape.png">'
                '<img src="/wrong/pic.png">'
                '<img src="//cdn.example.com/pic.png">'
                '<img src="mailto:user@example.com">'
                '<img src="https:missing-host.png">'
                '<source src="">'
                '<pre class="playground"><code class="language-rust">'
                '<span class="boring">unsafe { evil(); }\n</span>'
                "fn ok() {}\n"
                "</code></pre>"
                "</main></body></html>",
                encoding="utf-8",
            )
            errors = check_docs.check_built_html_links(
                html_root,
                verified_snippets=verified,
                verified_boring_lines=boring,
            )
            self.assertTrue(
                any("broken image/media source `missing.png`" in e for e in errors),
                errors,
            )
            self.assertTrue(
                any("broken image/media source `sub`" in e for e in errors),
                errors,
            )
            self.assertTrue(
                any("escapes built HTML root" in e for e in errors),
                errors,
            )
            self.assertTrue(
                any("absolute image/media source `/wrong/pic.png`" in e for e in errors),
                errors,
            )
            self.assertTrue(
                any("scheme-relative media source `//cdn.example.com/pic.png`" in e for e in errors),
                errors,
            )
            self.assertTrue(
                any("disallowed URL scheme `mailto:` in media source" in e for e in errors),
                errors,
            )
            self.assertTrue(
                any("malformed image/media source `https:missing-host.png`" in e for e in errors),
                errors,
            )
            self.assertTrue(
                any("empty image/media source `src`" in e for e in errors),
                errors,
            )
            self.assertTrue(
                any("rendered hidden `rust` line does not match" in e for e in errors),
                errors,
            )


class RepositoryDocsIntegrationTests(unittest.TestCase):
    def test_repository_book_sources_pass(self):
        self.assertEqual(check_docs.check_sources(ANNEAL_ROOT), [])

    def test_workflows_strip_actions_tokens_for_doc_steps(self):
        repo_root = ANNEAL_ROOT.parent
        anneal_ci = load_workflow_yaml(
            (repo_root / ".github/workflows/anneal-current.yml").read_text(
                encoding="utf-8"
            )
        )
        static_env = anneal_ci["jobs"]["static-checks"].get("env", {})
        for key in (
            "ACTIONS_CACHE_URL",
            "ACTIONS_RESULTS_URL",
            "ACTIONS_RUNTIME_TOKEN",
        ):
            self.assertEqual(static_env.get(key), "")

        test_verify_steps = anneal_ci["jobs"]["test-and-verify"]["steps"]
        self.assertEqual(
            [
                step
                for step in test_verify_steps
                if step.get("name") == "Verify and build public documentation"
            ],
            [],
        )

        smoke_steps = anneal_ci["jobs"]["installed-smoke"]["steps"]
        docs_steps = [
            step
            for step in smoke_steps
            if step.get("name") == "Verify and build public documentation"
        ]
        self.assertEqual(len(docs_steps), 1)
        env = docs_steps[0].get("env", {})
        for key in (
            "ACTIONS_CACHE_URL",
            "ACTIONS_RESULTS_URL",
            "ACTIONS_RUNTIME_TOKEN",
            "LD_LIBRARY_PATH",
        ):
            self.assertEqual(env.get(key), "")

        pages_wf = load_workflow_yaml(
            (repo_root / ".github/workflows/docs.yml").read_text(
                encoding="utf-8"
            )
        )
        build_steps = pages_wf["jobs"]["build"]["steps"]
        pages_docs_steps = [
            step
            for step in build_steps
            if step.get("name") == "Build Anneal public documentation"
        ]
        self.assertEqual(len(pages_docs_steps), 1)
        pages_env = pages_docs_steps[0].get("env", {})
        for key in (
            "ACTIONS_CACHE_URL",
            "ACTIONS_RESULTS_URL",
            "ACTIONS_RUNTIME_TOKEN",
        ):
            self.assertEqual(pages_env.get(key), "")
        self.assertIn("--output target/doc/anneal", pages_docs_steps[0]["run"])


class TutorWorkflowTests(unittest.TestCase):
    def _make_tutor_root(self, tmp_path: Path) -> Path:
        root = _make_minimal_anneal_root(tmp_path / "anneal")
        guide_dir = root / "book/src/guides"
        guide_dir.mkdir(parents=True, exist_ok=True)
        ex_dir = root / "book/examples/guides/demo"
        ex_dir.mkdir(parents=True, exist_ok=True)
        (ex_dir / "lib.rs").write_text("pub fn zero() -> u64 { 0 }\n", encoding="utf-8")
        (ex_dir / "broken-lib.rs").write_text(
            "pub fn zero() -> u64 { 1 }\n", encoding="utf-8"
        )
        (ex_dir / "request.json").write_text(
            '{"schema_version": 1}\n', encoding="utf-8"
        )
        (ex_dir / "Spec.lean.txt").write_text("-- spec\n", encoding="utf-8")
        (ex_dir / "Proof.lean.txt").write_text("-- proof\n", encoding="utf-8")
        (ex_dir / "init-transcript.txt").write_text(
            "$ cargo anneal init --standalone --proof-dir out\nok\n",
            encoding="utf-8",
        )
        (ex_dir / "pass-receipt.json").write_text(
            json.dumps(
                {
                    "schema_version": 3,
                    "status": "selected_unit_verified",
                    "experimental": True,
                    "elaboration_sandbox": "<linux_bwrap | linux_landlock_seccomp>",
                    "taint_reasons": [],
                    "spec_warnings": [],
                    "scope": {"kind": "selected_unit", "roots": ["zero"]},
                },
                indent=2,
            )
            + "\n",
            encoding="utf-8",
        )
        (ex_dir / "pass-transcript.txt").write_text(
            "$ cargo anneal verify --request request.json\n"
            + (ex_dir / "pass-receipt.json").read_text(encoding="utf-8"),
            encoding="utf-8",
        )
        (ex_dir / "fail-transcript.txt").write_text(
            "$ cargo anneal verify --request request.json\n"
            + json.dumps(
                {
                    "schema_version": 3,
                    "status": "proof_failed",
                    "phase": "audit",
                    "elaboration_sandbox": "<linux_bwrap | linux_landlock_seccomp>",
                    "taint_reasons": [],
                    "hint": "Proof.lean mismatch",
                },
                indent=2,
            )
            + "\n",
            encoding="utf-8",
        )
        (ex_dir / "gen-out.txt").write_text("generated-by-cli\n", encoding="utf-8")
        (ex_dir / "case.json").write_text(
            json.dumps(
                {
                    "kind": "tutor",
                    "description": "Tutor workflow smoke test",
                    "artifacts": [
                        {"file": "lib.rs", "role": "input", "stage_as": "lib.rs"},
                        {
                            "file": "request.json",
                            "role": "input",
                            "stage_as": "request.json",
                        },
                        {
                            "file": "Spec.lean.txt",
                            "role": "input",
                            "stage_as": "Spec.lean",
                        },
                        {
                            "file": "Proof.lean.txt",
                            "role": "input",
                            "stage_as": "Proof.lean",
                        },
                        {"file": "broken-lib.rs", "role": "mutation_source"},
                        {
                            "file": "gen-out.txt",
                            "role": "generated",
                            "workdir_path": "out/gen.txt",
                        },
                    ],
                    "steps": [
                        {
                            "display_command": (
                                "$ cargo anneal init --standalone --proof-dir out"
                            ),
                            "argv": [
                                "anneal",
                                "init",
                                "--standalone",
                                "--proof-dir",
                                "out",
                            ],
                            "expect_exit": 0,
                            "output_mode": "full",
                            "transcript": "init-transcript.txt",
                        },
                        {
                            "display_command": (
                                "$ cargo anneal verify --request request.json"
                            ),
                            "argv": ["anneal", "verify", "--request", "request.json"],
                            "expect_exit": 0,
                            "expect_receipt": {
                                "schema_version": 3,
                                "status": "selected_unit_verified",
                                "experimental": True,
                                "spec_warnings": [],
                                "scope": {"kind": "selected_unit", "roots": ["zero"]},
                            },
                            "receipt_excerpt": "pass-receipt.json",
                            "transcript": "pass-transcript.txt",
                            "output_mode": "receipt_summary",
                        },
                        {
                            "display_command": (
                                "$ cargo anneal verify --request request.json"
                            ),
                            "argv": ["anneal", "verify", "--request", "request.json"],
                            "copy_files": [{"src": "broken-lib.rs", "dest": "lib.rs"}],
                            "expect_exit": 1,
                            "expect_receipt": {
                                "schema_version": 3,
                                "status": "proof_failed",
                                "phase": "audit",
                                "hint_contains": "Proof.lean",
                            },
                            "transcript": "fail-transcript.txt",
                            "output_mode": "receipt_summary",
                        },
                    ],
                },
                indent=2,
            )
            + "\n",
            encoding="utf-8",
        )
        (guide_dir / "demo.md").write_text(
            "# Demo\n\n"
            "```rust\n{{#include ../../examples/guides/demo/lib.rs}}\n```\n\n"
            "```lean\n{{#include ../../examples/guides/demo/Spec.lean.txt}}\n"
            "```\n\n"
            "```json\n{{#include ../../examples/guides/demo/pass-receipt.json}}\n"
            "```\n\n"
            "```console\n"
            "{{#include ../../examples/guides/demo/fail-transcript.txt}}\n```\n",
            encoding="utf-8",
        )
        return root

    def _write_fake_cargo_anneal(
        self,
        path: Path,
        *,
        require_explicit_tools: bool = True,
        sandbox: str = "linux_bwrap",
        taint_reasons: list[str] | None = None,
        status_override: str | None = None,
        sleep_verify: bool = False,
    ) -> None:
        taints = list(taint_reasons or [])
        pass_status = status_override or "selected_unit_verified"
        script = (
            f"#!{sys.executable}\n"
            "import json, sys, time\n"
            "from pathlib import Path\n"
            "args = sys.argv[1:]\n"
            "if args and args[0] == 'anneal':\n"
            "    args = args[1:]\n"
            "if args == ['--help']:\n"
            "    print('Usage: cargo anneal')\n"
            "    sys.exit(0)\n"
            "if args[0] == 'init':\n"
            "    out = Path('out')\n"
            "    out.mkdir(exist_ok=True)\n"
            "    (out / 'gen.txt').write_text('generated-by-cli\\n')\n"
            "    print('ok')\n"
            "    sys.exit(0)\n"
            "if args[0] == 'verify':\n"
            f"    if {require_explicit_tools!r}:\n"
            "        assert '--capture' in args and '--sysroot' in args\n"
            "        assert '--lean' in args\n"
            "    else:\n"
            "        assert '--capture' not in args and '--sysroot' not in args\n"
            "        assert '--lean' not in args\n"
            f"    if {sleep_verify!r}:\n"
            "        time.sleep(2.0)\n"
            "    src = Path('lib.rs').read_text()\n"
            "    if '0' in src:\n"
            "        print(json.dumps({\n"
            "            'schema_version': 3,\n"
            f"            'status': {pass_status!r},\n"
            "            'experimental': True,\n"
            f"            'elaboration_sandbox': {sandbox!r},\n"
            f"            'taint_reasons': {taints!r},\n"
            "            'spec_warnings': [],\n"
            "            'scope': {'kind': 'selected_unit', 'roots': ['zero']},\n"
            "            'timing_ms': {'total': 5},\n"
            "        }))\n"
            "        sys.exit(0)\n"
            "    print(json.dumps({\n"
            "        'schema_version': 3,\n"
            "        'status': 'proof_failed',\n"
            "        'phase': 'audit',\n"
            f"        'elaboration_sandbox': {sandbox!r},\n"
            f"        'taint_reasons': {taints!r},\n"
            "        'hint': 'Proof.lean mismatch',\n"
            "    }))\n"
            "    sys.exit(1)\n"
        )
        path.write_text(script, encoding="utf-8")
        path.chmod(path.stat().st_mode | stat.S_IXUSR)

    def test_tutor_argv_rejections_and_schema_hardening_fail_closed(self):
        bad_argvs = [
            (
                "taint_unsandboxed",
                [
                    "anneal",
                    "verify",
                    "--request",
                    "r.json",
                    "--allow-unsandboxed-elaboration",
                ],
            ),
            (
                "taint_sorry",
                ["anneal", "verify", "--request", "r.json", "--allow-sorry-in-proof"],
            ),
            (
                "taint_unpinned",
                ["anneal", "verify", "--request", "r.json", "--allow-unpinned-lean"],
            ),
            (
                "manifest_path",
                [
                    "anneal",
                    "verify",
                    "--manifest-path",
                    "Cargo.toml",
                    "--proof-dir",
                    "p",
                ],
            ),
            ("escape_path", ["anneal", "verify", "--request", "../secret.json"]),
            ("bare_setup", ["anneal", "setup"]),
            (
                "dup_request",
                ["anneal", "verify", "--request", "a.json", "--request", "b.json"],
            ),
            (
                "mem_too_high",
                [
                    "anneal",
                    "verify",
                    "--request",
                    "r.json",
                    "--lean-memory-mb",
                    "999999",
                ],
            ),
            (
                "bad_rustc_target",
                [
                    "anneal",
                    "verify",
                    "rustc",
                    "src/lib.rs",
                    "--crate-name",
                    "c",
                    "--target",
                    "thumbv7m-none-eabi",
                    "--spec-file",
                    "Spec=proof/Spec.lean",
                    "--proof-file",
                    "Proof=proof/Proof.lean",
                ],
            ),
        ]
        for label, argv in bad_argvs:
            with self.subTest(case=label):
                self.assertIsNotNone(check_docs._validate_tutor_argv(argv))

        self.assertIsNone(
            check_docs._validate_tutor_argv(
                [
                    "anneal",
                    "verify",
                    "rustc",
                    "src/lib.rs",
                    "--crate-name",
                    "guest_width",
                    "--source-file",
                    "src/width.rs",
                    "--select",
                    "width::mask_width",
                    "--spec-file",
                    "Spec=proof/Spec.lean",
                    "--proof-file",
                    "Proof=proof/Proof.lean",
                ]
            )
        )

        with tempfile.TemporaryDirectory(prefix="anneal-tutor-stray-") as tmp:
            root = self._make_tutor_root(Path(tmp))
            ex_dir = root / "book/examples/guides/demo"
            (ex_dir / "stray.txt").write_text("stray\n", encoding="utf-8")
            errors = check_docs.check_sources(root)
            self.assertTrue(
                any("undeclared regular file" in e for e in errors), errors
            )
            (ex_dir / "stray.txt").unlink()

            base_obj = json.loads((ex_dir / "case.json").read_text(encoding="utf-8"))

            mut_cases = [
                (
                    "unknown_expect_key",
                    lambda o: o["steps"][1]["expect_receipt"].__setitem__(
                        "statuss", "typo"
                    ),
                    "unknown `expect_receipt` keys",
                ),
                (
                    "bad_sandbox_none",
                    lambda o: o["steps"][1]["expect_receipt"].__setitem__(
                        "elaboration_sandbox_in", ["none"]
                    ),
                    "elaboration_sandbox_in",
                ),
                (
                    "bad_schema_v2",
                    lambda o: o["steps"][1]["expect_receipt"].__setitem__(
                        "schema_version", 2
                    ),
                    "schema_version",
                ),
                (
                    "bad_status_unverified",
                    lambda o: o["steps"][1]["expect_receipt"].__setitem__(
                        "status", "unverified"
                    ),
                    "invalid `expect_receipt.status`",
                ),
                (
                    "bad_hint_int",
                    lambda o: o["steps"][2]["expect_receipt"].__setitem__(
                        "hint_contains", 5
                    ),
                    "hint_contains",
                ),
                (
                    "dot_case_target",
                    lambda o: o["steps"][0].__setitem__("transcript", "./case.json"),
                    "invalid artifact path",
                ),
                (
                    "dot_dup_artifact",
                    lambda o: o["artifacts"].append(
                        {"file": "./lib.rs", "role": "mutation_source"}
                    ),
                    "invalid artifact path",
                ),
                (
                    "wrong_role_key",
                    lambda o: o["artifacts"][4].__setitem__("stage_as", "x.rs"),
                    "invalid keys for role",
                ),
                (
                    "dup_stage_as",
                    lambda o: o["artifacts"].__setitem__(
                        1,
                        {
                            "file": "request.json",
                            "role": "input",
                            "stage_as": "lib.rs",
                        },
                    ),
                    "duplicate `stage_as`",
                ),
                (
                    "colliding_transcript",
                    lambda o: o["steps"][0].__setitem__(
                        "transcript", "pass-transcript.txt"
                    ),
                    "collides with existing artifact",
                ),
                (
                    "receipt_summary_missing_expect",
                    lambda o: o["steps"][0].__setitem__(
                        "output_mode", "receipt_summary"
                    ),
                    "requires `expect_receipt`",
                ),
                (
                    "step0_mutates_generated",
                    lambda o: o["steps"][0].__setitem__(
                        "copy_files", [{"src": "broken-lib.rs", "dest": "out/gen.txt"}]
                    ),
                    "cannot mutate a generated path before an earlier step creates it",
                ),
            ]
            for label, mutator, needle in mut_cases:
                with self.subTest(case=label):
                    obj = json.loads(json.dumps(base_obj))
                    mutator(obj)
                    (ex_dir / "case.json").write_text(
                        json.dumps(obj, indent=2) + "\n", encoding="utf-8"
                    )
                    errs = check_docs.check_sources(root)
                    self.assertTrue(any(needle in e for e in errs), (label, errs))

            # Non-ASCII digits (e.g. '²000') must fail cleanly without ValueError.
            self.assertIsNotNone(
                check_docs._validate_tutor_argv(
                    [
                        "anneal",
                        "verify",
                        "--request",
                        "r.json",
                        "--lean-memory-mb",
                        "\u00b2000",
                    ]
                )
            )

            # Direct check that _resolve_case_artifact rejects ./case.json.
            _, dot_case_err = check_docs._resolve_case_artifact(
                ex_dir / "case.json", "./case.json"
            )
            self.assertIsNotNone(dot_case_err)

    def test_tutor_case_runtime_checks_and_installed_bundle_branch(self):
        with tempfile.TemporaryDirectory(prefix="anneal-tutor-run-") as tmp:
            tmp_path = Path(tmp)
            root = self._make_tutor_root(tmp_path)
            ex_dir = root / "book/examples/guides/demo"
            self.assertEqual(check_docs.check_sources(root), [])

            cap = tmp_path / "cap"
            sysr = tmp_path / "sysroot"
            lean = tmp_path / "lean"
            cap.write_text("", encoding="utf-8")
            sysr.mkdir()
            lean.write_text("", encoding="utf-8")

            fake_cli = tmp_path / "fake-cargo-anneal"
            self._write_fake_cargo_anneal(fake_cli, require_explicit_tools=True)
            self.assertEqual(
                check_docs.verify_examples(
                    root, fake_cli, capture=cap, sysroot=sysr, lean=lean
                ),
                [],
            )

            # Both linux_bwrap and linux_landlock_seccomp must pass cleanly.
            self._write_fake_cargo_anneal(
                fake_cli,
                require_explicit_tools=True,
                sandbox="linux_landlock_seccomp",
                taint_reasons=[],
            )
            self.assertEqual(
                check_docs.verify_examples(
                    root, fake_cli, capture=cap, sysroot=sysr, lean=lean
                ),
                [],
            )

            # Mutating a generated file in step 2 must keep pre-mutation snapshot.
            mut_root = self._make_tutor_root(tmp_path / "mut_case")
            mut_ex = mut_root / "book/examples/guides/demo"
            self._write_fake_cargo_anneal(fake_cli, require_explicit_tools=True)
            (mut_ex / "mut-gen.txt").write_text("mutated!\n", encoding="utf-8")
            case_obj = json.loads((mut_ex / "case.json").read_text(encoding="utf-8"))
            case_obj["artifacts"].append(
                {"file": "mut-gen.txt", "role": "mutation_source"}
            )
            case_obj["steps"][2]["copy_files"].append(
                {"src": "mut-gen.txt", "dest": "out/gen.txt"}
            )
            (mut_ex / "case.json").write_text(
                json.dumps(case_obj, indent=2) + "\n", encoding="utf-8"
            )
            self.assertEqual(
                check_docs.verify_examples(
                    mut_root, fake_cli, capture=cap, sysroot=sysr, lean=lean
                ),
                [],
            )
            # Post-mutation content in gen-out.txt must fail verify_examples.
            (mut_ex / "gen-out.txt").write_text("mutated!\n", encoding="utf-8")
            gen_mismatch = check_docs.verify_examples(
                mut_root, fake_cli, capture=cap, sysroot=sysr, lean=lean
            )
            self.assertTrue(
                any("output mismatch" in e for e in gen_mismatch), gen_mismatch
            )
            ex_dir = root / "book/examples/guides/demo"

            # Untainted receipt without ExecSeal must fail the sandbox check.
            self._write_fake_cargo_anneal(
                fake_cli,
                require_explicit_tools=True,
                sandbox="none",
                taint_reasons=[],
            )
            sbx_errs = check_docs.verify_examples(
                root, fake_cli, capture=cap, sysroot=sysr, lean=lean
            )
            self.assertTrue(any("unexpected sandbox" in e for e in sbx_errs), sbx_errs)

            # Sandboxed receipt with taint_reasons must fail the taint check.
            self._write_fake_cargo_anneal(
                fake_cli,
                require_explicit_tools=True,
                sandbox="linux_bwrap",
                taint_reasons=["sorry_in_proof"],
            )
            taint_errs = check_docs.verify_examples(
                root, fake_cli, capture=cap, sysroot=sysr, lean=lean
            )
            self.assertTrue(
                any("unexpected taint" in e for e in taint_errs), taint_errs
            )

            # Runtime null status must not raise AttributeError in summary check.
            _, null_errs = check_docs._assert_and_summarize_receipt(
                json.dumps({"schema_version": 3, "status": None, "taint_reasons": []}),
                "",
                {"status": "selected_unit_verified"},
                ex_dir / "case.json",
                0,
                tmp_path,
                tmp_path,
                tmp_path,
            )
            self.assertTrue(any("expected status" in e for e in null_errs), null_errs)

            # Exercise main() installed-bundle CI branch & partial tool rejection.
            bundle_xdg = tmp_path / "ci-xdg"
            (bundle_xdg / "anneal" / "toolchain").mkdir(parents=True)
            self._write_fake_cargo_anneal(fake_cli, require_explicit_tools=False)
            old_env = os.environ.copy()
            try:
                os.environ["XDG_CACHE_HOME"] = str(bundle_xdg)
                for key in ("ANNEAL_CAPTURE", "ANNEAL_SYSROOT", "ANNEAL_LEAN"):
                    os.environ.pop(key, None)
                rc = check_docs.main(
                    [
                        "--anneal-root",
                        str(root),
                        "--cargo-anneal",
                        str(fake_cli),
                        "--verify-examples",
                    ]
                )
                self.assertEqual(rc, 0)

                os.environ["ANNEAL_CAPTURE"] = str(cap)
                rc_partial = check_docs.main(
                    [
                        "--anneal-root",
                        str(root),
                        "--cargo-anneal",
                        str(fake_cli),
                        "--verify-examples",
                    ]
                )
                self.assertEqual(rc_partial, 1)
            finally:
                os.environ.clear()
                os.environ.update(old_env)

            # TimeoutExpired must produce a clean error instead of traceback.
            self._write_fake_cargo_anneal(
                fake_cli, require_explicit_tools=True, sleep_verify=True
            )
            orig_run = check_docs.subprocess.run

            def _short_timeout_run(*args, **kwargs):
                kwargs["timeout"] = 0.05
                return orig_run(*args, **kwargs)

            with mock.patch.object(
                check_docs.subprocess, "run", side_effect=_short_timeout_run
            ):
                timeout_errs = check_docs.verify_examples(
                    root, fake_cli, capture=cap, sysroot=sysr, lean=lean
                )
            self.assertTrue(any("timed out" in e for e in timeout_errs), timeout_errs)


if __name__ == "__main__":
    unittest.main()
