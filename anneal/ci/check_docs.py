# Copyright 2026 The Fuchsia Authors
#
# Licensed under a BSD-style license <LICENSE-BSD>, Apache License, Version 2.0
# <LICENSE-APACHE or https://www.apache.org/licenses/LICENSE-2.0>, or the MIT
# license <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your option.
# This file may not be copied, modified, or distributed except according to
# those terms.

"""Validate, verify, and build Anneal's public-facing mdBook documentation.

This script enforces the anti-bitrot and containment invariants for
`anneal/book/`:

1. Closed, exact-case file-type allowlist under `anneal/book/`: Only
   `book.toml`, lowercase `.md` files under `book/src/`, and pure example
   artifacts (`case.json`, `.txt`, `.rs`, `.lock`, never `cargo.toml`/
   `cargo.lock`) under `book/examples/` are permitted. Scripts, Cargo
   manifests, Lean files, themes, templates, SVGs, and raw HTML files are
   rejected by default.
2. Default-deny inline code fences and post-build HTML parity: Markdown files
   under `book/src/` may not contain raw `rust`, `rs`, `lean`, `json`, `sh`,
   `bash`, `shell`, `zsh`, or `console` fenced code blocks. Every
   `{{#include ...}}` / `{{#rustdoc_include ...}}` directive must live inside a
   checked fence and target a registered verified artifact of a `case.json`
   under `book/examples/`. All other unescaped `{{#...}}` directives are
   rejected. After `mdbook build`, every rendered `<pre><code class="language-*">`
   block in authored chapters is also verified against the exact verified
   artifact texts, structurally closing CommonMark parser-desync bypasses.
3. Fail-closed anchor and hidden-spec checks: `mdbook v0.5.4` silently exits 0
   with an empty code block on missing `{{#include file:anchor}}` anchors. This
   script parses every include directive itself and rejects missing files,
   symlinks, path escapes, invalid or empty line ranges/anchors, and hidden
   `unsafe` or Anneal specification lines in anchored Rust examples.
4. Closed non-executing `book/book.toml` and HTML/script hygiene: Requires
   canonical `site-url = "/zerocopy/anneal/"`, restricts themes to mdBook's
   built-in themes, forbids `[preprocessor.*]` and custom `[output.*]`, rejects
   raw HTML tags outside comments/code in `book/src/*.md`, enforces exactly one
   `<main>...</main>` pair per rendered HTML page, rejects duplicate HTML
   attributes, disallowed tags (`script`, `iframe`, `svg`, `base`, `meta`,
   `link`, `form`, `object`, `embed`, `style`, `math`, `template`), `on*`
   handlers, and non-HTTP/mailto URL schemes on all URL-bearing attributes.
5. Closed CLI `argv` allowlist, single-sourced governing docs, and link checks:
   Restricts `case.json` `argv` to safe non-network/non-build `cargo-anneal`
   invocations (requiring explicit sandboxing before crate-building subcommands
   are admitted), verifies `display_command` matches `argv`, stages
   `PRINCIPLES.md`, `THREAT_MODEL.md`, and `META_PRINCIPLES.md` from `anneal/`
   with checked relative-link rewrites, and verifies built HTML links/anchors.
"""

from __future__ import annotations

import argparse
import difflib
import hashlib
import html
from html.parser import HTMLParser
import http.client
import json
import os
from pathlib import Path
import re
import shlex
import shutil
import subprocess
import sys
import tarfile
import tempfile
import time
import tomllib
from urllib.error import URLError
from urllib.parse import unquote, urlsplit
import urllib.request


MDBOOK_VERSION = "0.5.4"
MDBOOK_TARGET = "x86_64-unknown-linux-musl"
MDBOOK_URL = (
    f"https://github.com/rust-lang/mdBook/releases/download/"
    f"v{MDBOOK_VERSION}/mdbook-v{MDBOOK_VERSION}-{MDBOOK_TARGET}.tar.gz"
)
MDBOOK_SHA256 = (
    "5222beabd3e37dc5be0d18ff99b79058469354db5c220153a1b92db5ba12be89"
)
MDBOOK_BIN_SHA256 = (
    "ef05511c54795f5641c94017495b9e61df9a95652cd7f6e301a135e6e7e7dd9e"
)

SITE_URL_PREFIX = "/zerocopy/anneal/"
GITHUB_ZEROCOPY_PREFIX = "https://github.com/google/zerocopy/"
GITHUB_ANNEAL_BLOB_BASE = (
    "https://github.com/google/zerocopy/blob/main/anneal/"
)

ALLOWED_BOOK_TOML_SECTIONS = frozenset({"book", "output"})
ALLOWED_BOOK_KEYS = frozenset(
    {"title", "description", "authors", "language", "src"}
)
ALLOWED_OUTPUT_SECTIONS = frozenset({"html"})
ALLOWED_OUTPUT_HTML_KEYS = frozenset(
    {
        "site-url",
        "git-repository-url",
        "default-theme",
        "preferred-dark-theme",
        "no-section-label",
    }
)
ALLOWED_MDBOOK_THEMES = frozenset({"light", "rust", "coal", "navy", "ayu"})

# Closed exact-case file-extension allowlist under `anneal/book/examples/`.
ALLOWED_EXAMPLE_SUFFIXES = frozenset({".json", ".txt", ".rs", ".lock"})

CHECKED_FENCE_LANGS = frozenset(
    {
        "rust",
        "rs",
        "sh",
        "bash",
        "shell",
        "zsh",
        "console",
        "lean",
        "json",
    }
)
STRICT_CHAPTER_DIRS = frozenset({"getting-started", "guides", "reference"})

GOVERNING_DOCS = (
    ("PRINCIPLES.md", "foundations/principles.md"),
    ("THREAT_MODEL.md", "foundations/threat-model.md"),
    ("META_PRINCIPLES.md", "foundations/meta-principles.md"),
)

GOVERNING_LINK_REWRITES = {
    "PRINCIPLES.md": "principles.md",
    "THREAT_MODEL.md": "threat-model.md",
    "META_PRINCIPLES.md": "meta-principles.md",
    "docs/design.md": f"{GITHUB_ANNEAL_BLOB_BASE}docs/design.md",
    "docs/architecture.md": f"{GITHUB_ANNEAL_BLOB_BASE}docs/architecture.md",
    "docs/trust-model.md": f"{GITHUB_ANNEAL_BLOB_BASE}docs/trust-model.md",
    "docs/README.md": f"{GITHUB_ANNEAL_BLOB_BASE}docs/README.md",
}

STRICT_INCLUDE_RE = re.compile(
    r"^\{\{#(?P<kind>include|rustdoc_include)\s+(?P<spec>[^}\s\"']+)\}\}$"
)
ANY_DIRECTIVE_RE = re.compile(r"(?<!\\)\{\{#(?P<body>[^}]*)(?P<close>\}\})?")
FENCE_OPEN_RE = re.compile(
    r"^(?P<prefix>(?:[ ]{0,3}>[ ]?)*)(?P<list>(?:[ ]{0,3}(?:[-+*]|\d{1,9}[.)])[ \t]+)?)"
    r"(?P<indent>[ ]{0,3})(?P<fence>`{3,}|~{3,})(?P<info>.*)$"
)
# Any unparsed fence-like sequence outside a recognized fence block (e.g.
# nested containers such as `- > ```rust` or indented 4+ space fences).
STRAY_FENCE_RE = re.compile(r"^\s*(?:>\s*)*(?:(?:[-+*]|\d+[.)])\s+)*(?:`{3,}|~{3,})")
ANCHOR_START_RE = re.compile(r"ANCHOR:\s*([A-Za-z0-9_-]+)\b")
ANCHOR_END_RE = re.compile(r"ANCHOR_END:\s*([A-Za-z0-9_-]+)\b")
MD_LINK_RE = re.compile(r"(?<!!)\[[^\]]*\]\((?P<target>[^)\s]+)(?:\s+\"[^\"]*\")?\)", re.DOTALL)
MD_IMAGE_RE = re.compile(r"!\[[^\]]*\]\((?P<target>[^)]+)\)", re.DOTALL)
LOAD_BEARING_RUST_RE = re.compile(
    r"\bunsafe\b|anneal:|#!\[anneal::|#\[anneal::|(?:///|//!)\s*```anneal\b"
)
HTML_COMMENT_RE = re.compile(r"<!--.*?-->", re.DOTALL)
RAW_HTML_TAG_RE = re.compile(r"<(?:[A-Za-z/!])")
UNSAFE_SCHEME_RE = re.compile(r"\b(?:javascript|data|vbscript)\s*:", re.IGNORECASE)

DISALLOWED_MAIN_HTML_TAGS = frozenset(
    {
        "script",
        "iframe",
        "svg",
        "base",
        "meta",
        "link",
        "form",
        "object",
        "embed",
        "style",
        "math",
        "template",
    }
)
URL_ATTRS = frozenset(
    {"href", "src", "action", "formaction", "xlink:href", "srcdoc", "data"}
)
MEDIA_SRC_TAGS = frozenset({"img", "source", "audio", "video", "track"})
RUST_PLAYGROUND_BOILERPLATE_LINES = frozenset(
    {"#![allow(unused)]", "fn main() {", "}"}
)


class DocError(Exception):
    """Raised when a documentation check, example verification, or build fails."""


def _is_within(path: Path, root: Path) -> bool:
    try:
        path.resolve().relative_to(root.resolve())
        return True
    except ValueError:
        return False


def _has_symlink_component(path: Path, stop_at: Path) -> bool:
    """Return True if `path` or any ancestor below `stop_at` is a symlink."""
    stop_resolved = stop_at.resolve()
    cur = path
    while True:
        if cur.is_symlink():
            return True
        if cur == stop_resolved or cur == cur.parent:
            break
        try:
            if cur.resolve() == stop_resolved:
                break
        except OSError:
            return True
        cur = cur.parent
    return False


def _ensure_no_symlinks_under(root: Path, label: str, errors: list[str]) -> None:
    if not root.exists():
        errors.append(f"{label}: directory does not exist: {root}")
        return
    if root.is_symlink():
        errors.append(f"{label}: root directory is a symlink: {root}")
        return
    for dirpath, dirnames, filenames in os.walk(root, followlinks=False):
        current = Path(dirpath)
        for name in list(dirnames) + list(filenames):
            candidate = current / name
            if candidate.is_symlink():
                errors.append(f"{label}: symlink is not allowed: {candidate}")


def _strip_quote_prefix(line: str, quote_depth: int) -> str | None:
    """Strip `quote_depth` leading blockquote `>` markers from `line`."""
    rest = line
    for _ in range(quote_depth):
        stripped = rest.lstrip(" ")
        if not stripped.startswith(">"):
            return None
        rest = stripped[1:]
        if rest.startswith(" "):
            rest = rest[1:]
    return rest


def parse_fences(text: str) -> list[dict]:
    """Return all CommonMark fenced code blocks in Markdown `text`."""
    fences: list[dict] = []
    lines = text.splitlines()
    in_fence = False
    fence_char = ""
    fence_len = 0
    fence_info = ""
    fence_start = 0
    quote_depth = 0
    min_close_indent = 0
    max_close_indent = 3
    body_lines: list[tuple[int, str]] = []

    idx = 0
    while idx < len(lines):
        lineno = idx + 1
        line = lines[idx]
        if not in_fence:
            match = FENCE_OPEN_RE.match(line)
            if match:
                fence = match.group("fence")
                info = match.group("info").strip()
                if fence[0] == "`" and "`" in info:
                    idx += 1
                    continue
                in_fence = True
                fence_char = fence[0]
                fence_len = len(fence)
                fence_info = info
                fence_start = lineno
                quote_depth = match.group("prefix").count(">")
                list_prefix = match.group("list")
                if list_prefix:
                    min_close_indent = len(list_prefix)
                    max_close_indent = len(list_prefix) + 3
                else:
                    min_close_indent = 0
                    max_close_indent = 3
                body_lines = []
            idx += 1
        else:
            inner = _strip_quote_prefix(line, quote_depth)
            if inner is None:
                fences.append(
                    {
                        "start_line": fence_start,
                        "end_line": lineno - 1,
                        "info": fence_info,
                        "body": body_lines,
                        "unclosed": True,
                    }
                )
                in_fence = False
                # Re-process this line outside the exited blockquote.
                continue
            stripped = inner.lstrip(" ")
            indent = len(inner) - len(stripped)
            if stripped and indent < min_close_indent:
                # Exiting a list item terminates its fence immediately;
                # re-process the current line at top level.
                fences.append(
                    {
                        "start_line": fence_start,
                        "end_line": lineno - 1,
                        "info": fence_info,
                        "body": body_lines,
                        "unclosed": True,
                    }
                )
                in_fence = False
                continue
            if (
                min_close_indent <= indent <= max_close_indent
                and stripped
                and set(stripped.rstrip()) == {fence_char}
                and len(stripped.rstrip()) >= fence_len
            ):
                fences.append(
                    {
                        "start_line": fence_start,
                        "end_line": lineno,
                        "info": fence_info,
                        "body": body_lines,
                    }
                )
                in_fence = False
            else:
                body_lines.append((lineno, inner))
            idx += 1

    if in_fence:
        fences.append(
            {
                "start_line": fence_start,
                "end_line": len(lines),
                "info": fence_info,
                "body": body_lines,
                "unclosed": True,
            }
        )
    return fences


def _parse_fence_tokens(info: str) -> set[str]:
    """Return lowercase tokens from a fence info string (entity & backslash unescaped)."""
    if not info:
        return set()
    # Unescape CommonMark ASCII punctuation escapes (`\,`, `\\`, etc.) and HTML
    # character references so `rust\,no_run` or `&#114;ust` cannot evade checks.
    unescaped = re.sub(r"\\([!-/:-@\[-`{-~])", r"\1", html.unescape(info))
    tokens: set[str] = set()
    for part in re.split(r"[\s,]+", unescaped.strip()):
        if part:
            tokens.add(part.lower())
    return tokens


def _parse_include_spec(
    spec: str,
) -> tuple[str, str | None, tuple[int | None, int | None] | None]:
    """Parse `<path>`, `<path>:<anchor>`, or `<path>:<start>:<end>`."""
    parts = spec.split(":")
    if len(parts) == 1:
        return parts[0], None, None
    if len(parts) == 2:
        if parts[1].isascii() and parts[1].isdigit():
            line = int(parts[1])
            return parts[0], None, (line, line)
        if not parts[1]:
            raise DocError(f"empty anchor/line specifier: {spec!r}")
        return parts[0], parts[1], None
    if len(parts) == 3:
        for bound in (parts[1], parts[2]):
            if bound and not (bound.isascii() and bound.isdigit()):
                raise DocError(f"non-integer line range in include: {spec!r}")
        start = int(parts[1]) if parts[1] else None
        end = int(parts[2]) if parts[2] else None
        return parts[0], None, (start, end)
    raise DocError(f"malformed include specifier: {spec!r}")


def _extract_anchors(text: str) -> tuple[dict[str, tuple[int, int]], list[str]]:
    """Return `{anchor_name: (start_line_1based, end_line_1based)}` and errors."""
    errors: list[str] = []
    starts: dict[str, int] = {}
    anchors: dict[str, tuple[int, int]] = {}
    for lineno, line in enumerate(text.splitlines(), start=1):
        start_match = ANCHOR_START_RE.search(line)
        if start_match:
            name = start_match.group(1)
            if name in starts or name in anchors:
                errors.append(f"duplicate ANCHOR start `{name}` at line {lineno}")
            else:
                starts[name] = lineno
        end_match = ANCHOR_END_RE.search(line)
        if end_match:
            name = end_match.group(1)
            if name not in starts:
                errors.append(
                    f"ANCHOR_END `{name}` without matching ANCHOR at line {lineno}"
                )
            else:
                start_line = starts.pop(name)
                anchors[name] = (start_line, lineno)
    for name, start_line in sorted(starts.items()):
        errors.append(f"unclosed ANCHOR `{name}` opened at line {start_line}")
    return anchors, errors


def _strip_rust_hidden_lines(lines: list[str]) -> list[str]:
    """Apply mdBook's Rust `#` hiding and `##` unescaping rules to `lines`."""
    rendered: list[str] = []
    for line in lines:
        stripped = line.lstrip()
        if stripped == "#" or stripped.startswith("# "):
            continue
        if stripped.startswith("##"):
            indent = line[: len(line) - len(stripped)]
            rendered.append(indent + stripped[1:])
        else:
            rendered.append(line)
    return rendered


def _render_include_slice(
    target_text: str,
    kind: str,
    anchor: str | None,
    line_range: tuple[int | None, int | None] | None,
    *,
    lang: str = "",
) -> str:
    """Render the visible text that mdBook emits for one include directive."""
    lines = target_text.splitlines()
    if anchor is not None:
        anchors, _ = _extract_anchors(target_text)
        start_line, end_line = anchors[anchor]
        selected = [
            ln
            for ln in lines[start_line : end_line - 1]
            if not ANCHOR_START_RE.search(ln) and not ANCHOR_END_RE.search(ln)
        ]
    elif line_range is not None:
        start_opt, end_opt = line_range
        start = 1 if start_opt is None else start_opt
        end = len(lines) if end_opt is None else end_opt
        selected = lines[start - 1 : end]
    else:
        selected = lines

    if kind == "rustdoc_include" or lang in {"rust", "rs"}:
        selected = _strip_rust_hidden_lines(selected)
    return "\n".join(selected)


def _validate_rust_anchor_coverage(
    rs_path: Path,
    included_ranges: list[tuple[int, int, str]],
    errors: list[str],
) -> None:
    """Reject hidden `unsafe` or Anneal-spec lines outside visible includes."""
    lines = rs_path.read_text(encoding="utf-8").splitlines()
    visible: set[int] = set()
    for start, end, _kind in included_ranges:
        for idx in range(start, end + 1):
            if 1 <= idx <= len(lines):
                stripped = lines[idx - 1].lstrip()
                if stripped == "#" or stripped.startswith("# "):
                    continue
                visible.add(idx)

    for lineno, line in enumerate(lines, start=1):
        if lineno in visible:
            continue
        stripped = line.strip()
        if not stripped:
            continue
        if (
            stripped.startswith("//")
            and not stripped.startswith(("///", "//!"))
            and "anneal:" not in stripped
        ):
            continue
        if LOAD_BEARING_RUST_RE.search(line):
            errors.append(
                f"{rs_path}:{lineno}: load-bearing Rust/Anneal line is hidden "
                f"outside any visible included anchor: {stripped}"
            )


def _arg_contains_unsafe_path(arg: str) -> bool:
    """Return True if `arg` or its `=`-delimited value escapes relative scope."""
    candidates = [arg]
    if "=" in arg:
        candidates.append(arg.split("=", 1)[1])
    for value in candidates:
        if not value:
            continue
        if (
            value.startswith(("/", "~"))
            or Path(value).is_absolute()
            or ".." in Path(value).parts
        ):
            return True
    return False


def _display_command_matches_argv(display_command: str, argv: list[str]) -> bool:
    """Verify `display_command` matches the executed `cargo-anneal` `argv`."""
    if not display_command.startswith("$ "):
        return False
    try:
        shown = shlex.split(display_command[2:])
    except ValueError:
        return False
    if not shown:
        return False
    if shown[0] == "cargo":
        return len(shown) >= 2 and shown[1] == "anneal" and shown[1:] == argv
    if shown[0] == "cargo-anneal":
        return shown[1:] == argv
    return False


def _validate_cli_argv_allowlist(
    argv: list[str],
    expect_exit: int,
    fresh_cache_env: bool,
    setup_files: dict[str, str],
) -> str | None:
    """Enforce a closed allowlist of safe, offline `cargo-anneal` invocations.

    Rejects bare `setup` (including `setup --clear-old` without
    `--local-archive`, which would attempt remote downloads), rejects runtime
    `init` (which writes files), and rejects runtime `verify` invocations
    (other than `--help`/`-h`/`help`) until sandboxed execution and toolchain
    fixture isolation are wired for example verification.
    """
    args = argv[1:] if argv and argv[0] == "anneal" else argv
    if args in (
        [],
        ["-h"],
        ["--help"],
        ["-V"],
        ["--version"],
        ["help"],
        ["help", "setup"],
        ["setup", "-h"],
        ["setup", "--help"],
        ["help", "init"],
        ["init", "-h"],
        ["init", "--help"],
        ["help", "skill"],
        ["skill"],
        ["skill", "-h"],
        ["skill", "--help"],
        ["skill", "--example"],
        ["help", "verify"],
        ["verify", "-h"],
        ["verify", "--help"],
        ["verify", "help"],
        ["verify", "help", "rustc"],
        ["verify", "rustc", "-h"],
        ["verify", "rustc", "--help"],
    ):
        return None

    # Allow `setup --local-archive <file>` (with optional `--clear-old`) only
    # against a declared relative setup file inside a fresh cache directory
    # (never touching host state or network).
    if args and args[0] == "setup" and fresh_cache_env:
        rest = args[1:]
        if (
            len(rest) == 2
            and rest[0] == "--local-archive"
            and rest[1] in setup_files
        ):
            return None
        if len(rest) == 3 and "--clear-old" in rest:
            without_clear = [tok for tok in rest if tok != "--clear-old"]
            if (
                len(without_clear) == 2
                and without_clear[0] == "--local-archive"
                and without_clear[1] in setup_files
            ):
                return None

    # Allow explicit argument-parsing negative examples (exit 2) for unknown
    # single-token subcommands (excluding real or reserved subcommands).
    if (
        expect_exit == 2
        and len(args) == 1
        and re.fullmatch(r"[a-z][a-z0-9_-]*", args[0]) is not None
        and args[0] not in {"setup", "init", "skill", "verify", "help", "check", "build"}
    ):
        return None

    return (
        f"argv {argv!r} is outside the closed offline CLI allowlist; "
        f"sandboxed execution is required before admitting new subcommands"
    )


TUTOR_FORBIDDEN_FLAGS = frozenset(
    {
        "--allow-unpinned-lean",
        "--allow-unsandboxed-elaboration",
        "--allow-sorry-in-proof",
        "--no-cache",
        "--no-rebuild-bundled",
        "--manifest-path",
        "--local-archive",
        "--capture",
        "--sysroot",
        "--lean",
        "--export-lean-lib",
    }
)
TUTOR_ALLOWED_RUSTC_VALUE_FLAGS = frozenset(
    {
        "--crate-name",
        "--crate-type",
        "--panic",
        "--edition",
        "--target",
        "--cfg",
        "--env",
        "--source-file",
        "--spec-file",
        "--proof-file",
        "--spec-decl",
        "--theorem",
        "--select",
        "--artifacts",
        "--lean-memory-mb",
    }
)
ALLOWED_SANDBOXES = frozenset({"linux_bwrap", "linux_landlock_seccomp"})
ALLOWED_TUTOR_RECEIPT_STATUSES = frozenset(
    {
        "selected_unit_verified",
        "crate_verified",
        "proof_failed",
        "unsupported",
        "tool_failed",
    }
)
VERIFIED_TUTOR_STATUSES = frozenset({"selected_unit_verified", "crate_verified"})
TUTOR_SINGLE_VALUE_FLAGS = frozenset(
    {
        "--crate-name",
        "--crate-type",
        "--panic",
        "--edition",
        "--target",
        "--spec-decl",
        "--theorem",
        "--request",
        "--artifacts",
        "--lean-memory-mb",
    }
)
MAX_TUTOR_LEAN_MEMORY_MB = 65536


def _is_safe_relative_workpath(raw: str) -> bool:
    """Return True if `raw` is a clean relative workspace path."""
    if not raw or not isinstance(raw, str) or raw.startswith(("/", "~")):
        return False
    if "\\" in raw or any(seg in ("", ".", "..") for seg in raw.split("/")):
        return False
    p = Path(raw)
    if p.is_absolute() or not p.parts:
        return False
    if any(part in ("", ".", "..") for part in p.parts):
        return False
    name_l = p.name.lower()
    if name_l in {"cargo.toml", "cargo.lock"} or p.suffix.lower() in {".py", ".sh"}:
        return False
    return True


def _validate_lean_memory_mb(val: str) -> str | None:
    """Validate bounded `--lean-memory-mb` integer string."""
    if not re.fullmatch(r"[0-9]+", val):
        return f"--lean-memory-mb must be an integer, got {val!r}"
    mem_mb = int(val)
    if mem_mb < 4096 or mem_mb > MAX_TUTOR_LEAN_MEMORY_MB:
        return (
            f"--lean-memory-mb must be in 4096..{MAX_TUTOR_LEAN_MEMORY_MB}, "
            f"got {val!r}"
        )
    return None


def _validate_tutor_argv(argv: list[str]) -> str | None:
    """Validate a strict allowlist of offline/sandboxed tutor `cargo-anneal` cmds."""
    if not argv or argv[0] != "anneal":
        return f"tutor step argv must start with 'anneal': {argv!r}"
    args = argv[1:]
    for arg in argv:
        if _arg_contains_unsafe_path(arg):
            return f"tutor argv may not contain unsafe path: {arg!r}"
        flag = arg.split("=", 1)[0]
        if flag in TUTOR_FORBIDDEN_FLAGS:
            return f"tutor argv forbids flag {flag!r}: {argv!r}"

    if args[:2] == ["init", "--standalone"]:
        rest = args[2:]
        if not rest:
            return None
        if (
            len(rest) == 2
            and rest[0] == "--proof-dir"
            and _is_safe_relative_workpath(rest[1])
        ):
            return None
        return f"unsupported tutor `init` argv shape: {argv!r}"

    if args and args[0] == "verify":
        rest = args[1:]
        seen_single: set[str] = set()
        if rest and rest[0] == "rustc":
            if (
                len(rest) < 2
                or rest[1].startswith("-")
                or not _is_safe_relative_workpath(rest[1])
            ):
                return f"tutor `verify rustc` requires relative crate root: {argv!r}"
            idx = 2
            has_spec = False
            has_proof = False
            while idx < len(rest):
                flag = rest[idx]
                if flag not in TUTOR_ALLOWED_RUSTC_VALUE_FLAGS:
                    return (
                        f"unsupported flag {flag!r} in tutor `verify rustc`: {argv!r}"
                    )
                if flag in TUTOR_SINGLE_VALUE_FLAGS:
                    if flag in seen_single:
                        return f"duplicate {flag!r} in tutor `verify rustc`: {argv!r}"
                    seen_single.add(flag)
                if idx + 1 >= len(rest):
                    return (
                        f"missing value for {flag!r} in tutor `verify rustc`: {argv!r}"
                    )
                val = rest[idx + 1]
                if flag == "--edition" and val not in {"2015", "2018", "2021", "2024"}:
                    return f"unsupported edition {val!r} in tutor `verify rustc`"
                if flag == "--target" and val != "x86_64-unknown-linux-gnu":
                    return f"unsupported target {val!r} in tutor `verify rustc`"
                if flag == "--crate-type" and val not in {"lib", "bin"}:
                    return f"unsupported crate-type {val!r} in tutor `verify rustc`"
                if flag == "--panic" and val not in {"unwind", "abort"}:
                    return f"unsupported panic {val!r} in tutor `verify rustc`"
                if flag in {"--source-file", "--artifacts"}:
                    if not _is_safe_relative_workpath(val):
                        return f"invalid relative path {val!r} for {flag!r}"
                if flag in {"--spec-file", "--proof-file"}:
                    if "=" not in val:
                        return f"{flag!r} expects MOD=PATH, got {val!r}"
                    _mod, path_val = val.split("=", 1)
                    if not _mod or not _is_safe_relative_workpath(path_val):
                        return f"invalid {flag!r} mapping: {val!r}"
                    if flag == "--spec-file":
                        has_spec = True
                    else:
                        has_proof = True
                if flag == "--lean-memory-mb":
                    mem_err = _validate_lean_memory_mb(val)
                    if mem_err:
                        return mem_err
                idx += 2
            if not (has_spec and has_proof):
                return (
                    f"tutor `verify rustc` requires --spec-file and --proof-file: "
                    f"{argv!r}"
                )
            return None

        idx = 0
        has_request = False
        while idx < len(rest):
            flag = rest[idx]
            if flag not in {"--request", "--artifacts", "--lean-memory-mb"}:
                return f"unsupported flag {flag!r} in tutor `verify`: {argv!r}"
            if flag in seen_single:
                return f"duplicate {flag!r} in tutor `verify`: {argv!r}"
            seen_single.add(flag)
            if idx + 1 >= len(rest):
                return f"missing value for {flag!r} in tutor `verify`: {argv!r}"
            val = rest[idx + 1]
            if flag in {"--request", "--artifacts"}:
                if not _is_safe_relative_workpath(val):
                    return f"invalid relative path {val!r} for {flag!r}"
            if flag == "--request":
                has_request = True
            if flag == "--lean-memory-mb":
                mem_err = _validate_lean_memory_mb(val)
                if mem_err:
                    return mem_err
            idx += 2
        if not has_request:
            return f"tutor `verify` requires --request: {argv!r}"
        return None

    return f"argv {argv!r} is outside the closed tutor CLI allowlist"


def _resolve_case_artifact(
    case_file: Path, rel_str: str, *, allow_missing: bool = False
) -> tuple[Path | None, str | None]:
    """Validate and resolve a relative artifact path inside `case_file.parent`."""
    if not isinstance(rel_str, str) or not rel_str:
        return None, f"invalid artifact path: {rel_str!r}"
    if "\\" in rel_str or any(seg in ("", ".", "..") for seg in rel_str.split("/")):
        return None, f"invalid artifact path: {rel_str!r}"
    p = Path(rel_str)
    if p.is_absolute() or ".." in p.parts:
        return None, f"artifact path escapes case directory: {rel_str!r}"
    raw = case_file.parent / rel_str
    if _has_symlink_component(raw, case_file.parent):
        return None, f"artifact path traverses symlink: {rel_str!r}"
    resolved = raw.resolve()
    if resolved == case_file.resolve():
        return None, f"artifact path cannot refer to case.json: {rel_str!r}"
    if not _is_within(resolved, case_file.parent):
        return None, f"artifact escapes case directory: {rel_str!r}"
    if not allow_missing and not resolved.is_file():
        return None, f"missing artifact file: {resolved}"
    return resolved, None


def _validate_expect_receipt_spec(
    case_file: Path, idx: int, exp_r: object, allowed_keys: set[str]
) -> list[str]:
    """Type- and value-check a step's `expect_receipt` expectations."""
    errors: list[str] = []
    if not isinstance(exp_r, dict):
        return [f"{case_file}: step[{idx}] `expect_receipt` must be an object"]
    unknown_r = set(exp_r.keys()) - allowed_keys
    if unknown_r:
        errors.append(
            f"{case_file}: step[{idx}] unknown `expect_receipt` keys: "
            f"{sorted(unknown_r)}"
        )
    if "schema_version" in exp_r:
        sv = exp_r["schema_version"]
        if isinstance(sv, bool) or sv != 3:
            errors.append(
                f"{case_file}: step[{idx}] `expect_receipt.schema_version` must be 3"
            )
    if "status" in exp_r:
        st = exp_r["status"]
        if not isinstance(st, str) or st not in ALLOWED_TUTOR_RECEIPT_STATUSES:
            errors.append(
                f"{case_file}: step[{idx}] invalid `expect_receipt.status` {st!r}"
            )
    if "experimental" in exp_r and not isinstance(exp_r["experimental"], bool):
        errors.append(
            f"{case_file}: step[{idx}] `expect_receipt.experimental` must be bool"
        )
    if "elaboration_sandbox_in" in exp_r:
        sbx_list = exp_r["elaboration_sandbox_in"]
        if (
            not isinstance(sbx_list, list)
            or not sbx_list
            or not all(isinstance(x, str) and x in ALLOWED_SANDBOXES for x in sbx_list)
        ):
            errors.append(
                f"{case_file}: step[{idx}] `elaboration_sandbox_in` must be non-empty "
                f"subset of {sorted(ALLOWED_SANDBOXES)}"
            )
    for str_key in ("phase", "hint_contains", "detail_contains", "stderr_contains"):
        if str_key in exp_r:
            val = exp_r[str_key]
            if not isinstance(val, str) or not val:
                errors.append(
                    f"{case_file}: step[{idx}] `expect_receipt.{str_key}` must be "
                    f"non-empty string"
                )
    if "spec_warnings" in exp_r and not isinstance(exp_r["spec_warnings"], list):
        errors.append(
            f"{case_file}: step[{idx}] `expect_receipt.spec_warnings` must be a list"
        )
    for list_key in ("assumptions", "deferred", "selected_paths"):
        if list_key in exp_r:
            vals = exp_r[list_key]
            if not isinstance(vals, list) or not all(isinstance(x, str) for x in vals):
                errors.append(
                    f"{case_file}: step[{idx}] `expect_receipt.{list_key}` must be "
                    f"list of strings"
                )
    if "scope" in exp_r and not isinstance(exp_r["scope"], dict):
        errors.append(
            f"{case_file}: step[{idx}] `expect_receipt.scope` must be an object"
        )
    return errors


def _validate_tutor_case(
    case_file: Path, data: dict
) -> tuple[set[Path], list[str]]:
    """Validate a `"kind": "tutor"` workflow case and return registered artifacts."""
    errors: list[str] = []
    verified: set[Path] = set()
    allowed_keys = {"kind", "description", "artifacts", "steps"}
    unknown = set(data.keys()) - allowed_keys
    if unknown:
        errors.append(f"{case_file}: unknown fields in tutor case: {sorted(unknown)}")
    for req in ("description", "artifacts", "steps"):
        if req not in data:
            errors.append(f"{case_file}: missing required field `{req}`")

    artifacts = data.get("artifacts")
    declared_paths: set[Path] = set()
    copy_sources: set[Path] = set()
    output_paths: set[Path] = set()
    seen_stage_as: set[str] = set()
    seen_workdir_paths: set[str] = set()
    role_allowed_keys = {
        "input": {"file", "role", "stage_as"},
        "generated": {"file", "role", "workdir_path"},
        "mutation_source": {"file", "role"},
    }
    if not isinstance(artifacts, list) or not artifacts:
        errors.append(f"{case_file}: `artifacts` must be a non-empty list")
    else:
        for entry in artifacts:
            if not isinstance(entry, dict):
                errors.append(f"{case_file}: each artifact entry must be an object")
                continue
            rel_file = entry.get("file")
            role = entry.get("role")
            if role not in role_allowed_keys:
                errors.append(f"{case_file}: invalid artifact role {role!r}")
                continue
            unknown_art = set(entry.keys()) - role_allowed_keys[role]
            if unknown_art:
                errors.append(
                    f"{case_file}: invalid keys for role {role!r}: "
                    f"{sorted(unknown_art)}"
                )
            resolved, err = _resolve_case_artifact(case_file, rel_file)
            if err:
                errors.append(f"{case_file}: {err}")
            elif resolved is not None:
                if resolved in declared_paths:
                    errors.append(f"{case_file}: duplicate artifact file {rel_file!r}")
                declared_paths.add(resolved)
                verified.add(resolved)
                if role in {"input", "mutation_source"}:
                    copy_sources.add(resolved)
                if role == "generated":
                    output_paths.add(resolved)
            if role == "input":
                stage_as = entry.get("stage_as")
                if not isinstance(stage_as, str) or not _is_safe_relative_workpath(
                    stage_as
                ):
                    errors.append(f"{case_file}: invalid `stage_as` path {stage_as!r}")
                elif stage_as in seen_stage_as:
                    errors.append(
                        f"{case_file}: duplicate `stage_as` target {stage_as!r}"
                    )
                else:
                    seen_stage_as.add(stage_as)
            elif role == "generated":
                wd_path = entry.get("workdir_path")
                if not isinstance(wd_path, str) or not _is_safe_relative_workpath(
                    wd_path
                ):
                    errors.append(f"{case_file}: invalid `workdir_path` {wd_path!r}")
                elif wd_path in seen_workdir_paths:
                    errors.append(
                        f"{case_file}: duplicate `workdir_path` target {wd_path!r}"
                    )
                else:
                    seen_workdir_paths.add(wd_path)

    steps = data.get("steps")
    allowed_expect_receipt_keys = {
        "schema_version",
        "status",
        "experimental",
        "elaboration_sandbox_in",
        "phase",
        "spec_warnings",
        "scope",
        "assumptions",
        "deferred",
        "selected_paths",
        "hint_contains",
        "detail_contains",
        "stderr_contains",
    }
    if not isinstance(steps, list) or not steps:
        errors.append(f"{case_file}: `steps` must be a non-empty list")
    else:
        allowed_step_keys = {
            "display_command",
            "argv",
            "expect_exit",
            "copy_files",
            "expect_receipt",
            "transcript",
            "receipt_excerpt",
            "output_mode",
        }
        for idx, step in enumerate(steps):
            if not isinstance(step, dict):
                errors.append(f"{case_file}: step[{idx}] must be an object")
                continue
            unknown_s = set(step.keys()) - allowed_step_keys
            if unknown_s:
                errors.append(
                    f"{case_file}: step[{idx}] unknown keys: {sorted(unknown_s)}"
                )
            argv = step.get("argv")
            argv_ok = isinstance(argv, list) and all(isinstance(a, str) for a in argv)
            if not argv_ok:
                errors.append(
                    f"{case_file}: step[{idx}] `argv` must be list of strings"
                )
            else:
                allow_err = _validate_tutor_argv(argv)
                if allow_err:
                    errors.append(f"{case_file}: step[{idx}] {allow_err}")
            display_cmd = step.get("display_command")
            if not isinstance(display_cmd, str) or not display_cmd.startswith("$ "):
                errors.append(
                    f"{case_file}: step[{idx}] `display_command` must start with `$ `"
                )
            elif argv_ok and not _display_command_matches_argv(display_cmd, argv):
                errors.append(
                    f"{case_file}: step[{idx}] `display_command` mismatches `argv`"
                )
            expect_exit = step.get("expect_exit")
            if isinstance(expect_exit, bool) or not isinstance(expect_exit, int):
                errors.append(f"{case_file}: step[{idx}] `expect_exit` must be an int")

            output_mode = step.get("output_mode")
            if output_mode is not None and output_mode not in {
                "full",
                "receipt_summary",
            }:
                errors.append(
                    f"{case_file}: step[{idx}] invalid `output_mode` {output_mode!r}"
                )

            exp_r = step.get("expect_receipt")
            if exp_r is not None:
                errors.extend(
                    _validate_expect_receipt_spec(
                        case_file, idx, exp_r, allowed_expect_receipt_keys
                    )
                )
            if argv_ok and len(argv) >= 2 and argv[1] == "verify":
                if not isinstance(exp_r, dict) or not exp_r.get("status"):
                    errors.append(
                        f"{case_file}: step[{idx}] `verify` requires "
                        f"`expect_receipt.status`"
                    )
            if "receipt_excerpt" in step and exp_r is None:
                errors.append(
                    f"{case_file}: step[{idx}] `receipt_excerpt` requires "
                    f"`expect_receipt`"
                )
            if output_mode == "receipt_summary" and exp_r is None:
                errors.append(
                    f"{case_file}: step[{idx}] `output_mode` 'receipt_summary' "
                    f"requires `expect_receipt`"
                )

            copy_files = step.get("copy_files") or []
            if not isinstance(copy_files, list):
                errors.append(f"{case_file}: step[{idx}] `copy_files` must be a list")
            else:
                for cp_entry in copy_files:
                    if not isinstance(cp_entry, dict):
                        errors.append(
                            f"{case_file}: step[{idx}] invalid `copy_files` item"
                        )
                        continue
                    src = cp_entry.get("src")
                    dest = cp_entry.get("dest")
                    src_res, src_err = _resolve_case_artifact(case_file, src)
                    if src_err or src_res is None or src_res not in copy_sources:
                        errors.append(
                            f"{case_file}: step[{idx}] unregistered copy src {src!r}"
                        )
                    if (
                        not isinstance(dest, str)
                        or not _is_safe_relative_workpath(dest)
                    ):
                        errors.append(
                            f"{case_file}: step[{idx}] invalid copy dest {dest!r}"
                        )
                    elif idx == 0 and dest in seen_workdir_paths:
                        errors.append(
                            f"{case_file}: step[0] copy dest {dest!r} cannot mutate "
                            f"a generated path before an earlier step creates it"
                        )

            has_output = False
            for out_key in ("transcript", "receipt_excerpt"):
                if out_key in step:
                    has_output = True
                    out_rel = step[out_key]
                    resolved, err = _resolve_case_artifact(case_file, out_rel)
                    if err:
                        errors.append(f"{case_file}: step[{idx}] {out_key}: {err}")
                    elif resolved is not None:
                        if resolved in declared_paths or resolved in output_paths:
                            errors.append(
                                f"{case_file}: step[{idx}] {out_key} collides with "
                                f"existing artifact {out_rel!r}"
                            )
                        output_paths.add(resolved)
                        verified.add(resolved)
            if not has_output and "expect_receipt" not in step:
                errors.append(
                    f"{case_file}: step[{idx}] must verify at least one output or "
                    f"receipt"
                )

    case_dir = case_file.parent.resolve()
    for p in sorted(case_dir.rglob("*")):
        if p.is_dir() or p.resolve() == case_file.resolve():
            continue
        if p.resolve() not in verified:
            rel = p.relative_to(case_dir)
            errors.append(
                f"{case_file}: undeclared regular file in tutor directory: {rel}"
            )
    return verified, errors


def _load_example_cases(
    examples_root: Path,
) -> tuple[list[tuple[Path, dict, set[Path]]], list[str]]:
    """Auto-discover and validate all `case.json` manifests under `examples/`."""
    errors: list[str] = []
    cases: list[tuple[Path, dict, set[Path]]] = []
    if not examples_root.exists():
        return cases, [f"examples directory missing: {examples_root}"]
    if examples_root.is_symlink():
        return cases, [f"examples directory may not be a symlink: {examples_root}"]

    case_files = sorted(examples_root.rglob("case.json"))
    if not case_files:
        return cases, [f"no example case.json files found under {examples_root}"]

    allowed_cli_keys = {
        "kind",
        "description",
        "display_command",
        "argv",
        "expect_exit",
        "transcript",
        "setup_files",
        "fresh_cache_env",
        "normalize_tempdir",
    }

    for case_file in case_files:
        if case_file.is_symlink():
            errors.append(f"{case_file}: symlink is not allowed")
            continue
        try:
            data = json.loads(case_file.read_text(encoding="utf-8"))
        except (OSError, json.JSONDecodeError) as exc:
            errors.append(f"{case_file}: invalid JSON: {exc}")
            continue
        if not isinstance(data, dict):
            errors.append(f"{case_file}: root JSON value must be an object")
            continue
        kind = data.get("kind")
        if kind == "tutor":
            verified_artifacts, tutor_errors = _validate_tutor_case(case_file, data)
            errors.extend(tutor_errors)
            cases.append((case_file, data, verified_artifacts))
            continue
        if kind != "cli":
            errors.append(f"{case_file}: unsupported example kind: {kind!r}")
            continue

        unknown = set(data.keys()) - allowed_cli_keys
        if unknown:
            errors.append(
                f"{case_file}: unknown fields in cli case: {sorted(unknown)}"
            )
        for req in ("display_command", "argv", "expect_exit", "transcript"):
            if req not in data:
                errors.append(f"{case_file}: missing required field `{req}`")

        argv = data.get("argv")
        argv_ok = isinstance(argv, list) and all(isinstance(a, str) for a in argv)
        if not argv_ok:
            errors.append(f"{case_file}: `argv` must be a list of strings")
        else:
            for arg in argv:
                if _arg_contains_unsafe_path(arg):
                    errors.append(
                        f"{case_file}: `argv` may not contain absolute or "
                        f"parent-traversal paths: {arg!r}"
                    )

        expect_exit = data.get("expect_exit")
        exit_ok = not isinstance(expect_exit, bool) and isinstance(expect_exit, int)
        if not exit_ok:
            errors.append(f"{case_file}: `expect_exit` must be an integer")

        display_command = data.get("display_command")
        if not isinstance(display_command, str) or not display_command.startswith("$ "):
            errors.append(f"{case_file}: `display_command` must start with `$ `")
        elif argv_ok and not _display_command_matches_argv(display_command, argv):
            errors.append(
                f"{case_file}: `display_command` ({display_command!r}) does not "
                f"match `argv` ({argv!r})"
            )

        for bool_field in ("fresh_cache_env", "normalize_tempdir"):
            if bool_field in data and not isinstance(data[bool_field], bool):
                errors.append(f"{case_file}: `{bool_field}` must be a boolean")

        setup_files_clean: dict[str, str] = {}
        if "setup_files" in data:
            setup_files = data["setup_files"]
            if not isinstance(setup_files, dict):
                errors.append(f"{case_file}: `setup_files` must be an object")
            else:
                for rel_name, content in setup_files.items():
                    if not isinstance(rel_name, str) or not isinstance(content, str):
                        errors.append(
                            f"{case_file}: `setup_files` entries must map strings to strings"
                        )
                        continue
                    rel_path = Path(rel_name)
                    if (
                        not rel_name
                        or not rel_path.parts
                        or rel_path.is_absolute()
                        or ".." in rel_path.parts
                    ):
                        errors.append(
                            f"{case_file}: invalid `setup_files` relative path: {rel_name!r}"
                        )
                    else:
                        setup_files_clean[rel_name] = content

        if argv_ok and exit_ok:
            allow_err = _validate_cli_argv_allowlist(
                argv,
                expect_exit,
                data.get("fresh_cache_env") is True,
                setup_files_clean,
            )
            if allow_err is not None:
                errors.append(f"{case_file}: {allow_err}")

        verified_artifacts: set[Path] = set()
        transcript = data.get("transcript")
        if not isinstance(transcript, str) or not transcript:
            errors.append(f"{case_file}: `transcript` must be a relative path")
        else:
            raw_transcript = case_file.parent / transcript
            if _has_symlink_component(raw_transcript, case_file.parent):
                errors.append(f"{case_file}: `transcript` may not traverse symlinks")
            else:
                transcript_path = raw_transcript.resolve()
                if not _is_within(transcript_path, case_file.parent):
                    errors.append(
                        f"{case_file}: `transcript` escapes case directory: {transcript}"
                    )
                elif not transcript_path.is_file():
                    errors.append(
                        f"{case_file}: missing transcript file: {transcript_path}"
                    )
                else:
                    verified_artifacts.add(transcript_path)
        cases.append((case_file, data, verified_artifacts))

    return cases, errors


def _check_prose_for_local_images(
    prose_text: str, rel_to_anneal: Path, errors: list[str]
) -> None:
    """Reject local or non-HTTP(S) Markdown image targets in prose."""
    without_comments = HTML_COMMENT_RE.sub(
        lambda m: "\n" * m.group(0).count("\n"), prose_text
    )
    without_code = _strip_inline_code_spans(without_comments)
    for match in MD_IMAGE_RE.finditer(without_code):
        raw_target = match.group("target").strip()
        # Strip optional Markdown link title (`path "title"`) and angle brackets.
        target_token = raw_target.split(None, 1)[0] if raw_target else ""
        if target_token.startswith("<") and target_token.endswith(">"):
            target_token = target_token[1:-1].strip()
        parsed = urlsplit(target_token)
        scheme = parsed.scheme.lower()
        if scheme not in {"http", "https"} or not parsed.netloc:
            errors.append(
                f"{rel_to_anneal}: disallowed or local Markdown image target "
                f"`{raw_target}` (local image assets are not permitted)"
            )


def _rewrite_governing_doc(
    source_path: Path,
    dest_rel: str,
    anneal_root: Path,
) -> tuple[str, list[str]]:
    """Validate and rewrite root-relative links in a governing doc for staging."""
    text = source_path.read_text(encoding="utf-8")
    rel_src = source_path.relative_to(anneal_root)
    errors: list[str] = []

    def replace_link(match: re.Match[str]) -> str:
        raw_target = match.group("target")
        if raw_target.startswith(("http://", "https://", "mailto:", "#")):
            return match.group(0)
        path_part, sep, frag = raw_target.partition("#")
        if path_part in GOVERNING_LINK_REWRITES:
            rewritten = GOVERNING_LINK_REWRITES[path_part]
            if sep:
                rewritten = f"{rewritten}#{frag}"
            return match.group(0).replace(f"({raw_target})", f"({rewritten})", 1)
        errors.append(
            f"{rel_src}: unmapped relative link "
            f"`{raw_target}` when staging to `{dest_rel}`; add it to "
            f"GOVERNING_LINK_REWRITES in ci/check_docs.py"
        )
        return match.group(0)

    fences = parse_fences(text)
    for fence in fences:
        if fence.get("unclosed"):
            errors.append(
                f"{rel_src}:{fence['start_line']}: unclosed fenced code block "
                f"in staged governing doc"
            )
    fence_ranges = [(f["start_line"], f["end_line"]) for f in fences]
    lines = text.splitlines(keepends=True)

    def line_in_fence(lineno: int) -> bool:
        return any(start <= lineno <= end for start, end in fence_ranges)

    # Reject mdBook directives on ALL lines (including inside fences, where
    # mdBook still expands `{{#include ...}}`) because staged governing docs
    # are excluded from post-build code-parity checks.
    for lineno, raw_line in enumerate(lines, start=1):
        line_no_nl = raw_line.rstrip("\r\n")
        for match in ANY_DIRECTIVE_RE.finditer(line_no_nl):
            errors.append(
                f"{rel_src}:{lineno}: mdBook directive `{match.group(0)}` is "
                f"forbidden in staged governing doc"
            )
        if not line_in_fence(lineno) and STRAY_FENCE_RE.match(line_no_nl):
            errors.append(
                f"{rel_src}:{lineno}: nested or indented code fence marker "
                f"outside a top-level fence is not allowed in staged governing doc"
            )

    prose_only_lines = [
        "" if line_in_fence(lineno) else raw_line.rstrip("\r\n")
        for lineno, raw_line in enumerate(lines, start=1)
    ]
    prose_text = "\n".join(prose_only_lines)
    _check_prose_for_raw_html(prose_text, rel_src, errors)
    _check_prose_for_local_images(prose_text, rel_src, errors)

    out_chunks: list[str] = []
    prose_buf: list[str] = []

    def flush_prose() -> None:
        if prose_buf:
            out_chunks.append(MD_LINK_RE.sub(replace_link, "".join(prose_buf)))
            prose_buf.clear()

    for lineno, line in enumerate(lines, start=1):
        if line_in_fence(lineno):
            flush_prose()
            out_chunks.append(line)
        else:
            prose_buf.append(line)
    flush_prose()

    return "".join(out_chunks), errors


def _check_book_toml(book_root: Path, anneal_root: Path, errors: list[str]) -> None:
    """Validate `book/book.toml` against a closed non-executing key/value allowlist."""
    toml_path = book_root / "book.toml"
    if toml_path.is_symlink() or not toml_path.is_file():
        errors.append(f"missing book manifest: {toml_path.relative_to(anneal_root)}")
        return
    try:
        config = tomllib.loads(toml_path.read_text(encoding="utf-8"))
    except (OSError, tomllib.TOMLDecodeError) as exc:
        errors.append(f"book/book.toml: invalid TOML: {exc}")
        return

    unknown_top = set(config.keys()) - ALLOWED_BOOK_TOML_SECTIONS
    if unknown_top:
        errors.append(
            f"book/book.toml: disallowed top-level sections "
            f"(preprocessors and build overrides are forbidden): {sorted(unknown_top)}"
        )

    book_sec = config.get("book")
    if not isinstance(book_sec, dict):
        errors.append("book/book.toml: missing `[book]` table")
    else:
        unknown_book = set(book_sec.keys()) - ALLOWED_BOOK_KEYS
        if unknown_book:
            errors.append(
                f"book/book.toml: disallowed `[book]` keys: {sorted(unknown_book)}"
            )
        src_val = book_sec.get("src", "src")
        if src_val != "src":
            errors.append(
                f'book/book.toml: `[book].src` must be `"src"`, got {src_val!r}'
            )
        for str_key in ("title", "description", "language"):
            if str_key in book_sec and not isinstance(book_sec[str_key], str):
                errors.append(f"book/book.toml: `[book].{str_key}` must be a string")
        if "authors" in book_sec and (
            not isinstance(book_sec["authors"], list)
            or not all(isinstance(a, str) for a in book_sec["authors"])
        ):
            errors.append("book/book.toml: `[book].authors` must be a list of strings")

    output_sec = config.get("output")
    if not isinstance(output_sec, dict):
        errors.append("book/book.toml: missing `[output]` table")
        return
    unknown_out = set(output_sec.keys()) - ALLOWED_OUTPUT_SECTIONS
    if unknown_out:
        errors.append(
            f"book/book.toml: disallowed `[output]` sections: "
            f"{sorted(unknown_out)}"
        )
    html_sec = output_sec.get("html")
    if not isinstance(html_sec, dict):
        errors.append("book/book.toml: missing `[output.html]` table")
        return
    unknown_html = set(html_sec.keys()) - ALLOWED_OUTPUT_HTML_KEYS
    if unknown_html:
        errors.append(
            f"book/book.toml: disallowed `[output.html]` keys: "
            f"{sorted(unknown_html)}"
        )
    if html_sec.get("site-url") != SITE_URL_PREFIX:
        errors.append(
            f"book/book.toml: `[output.html].site-url` must be "
            f"{SITE_URL_PREFIX!r}, got {html_sec.get('site-url')!r}"
        )
    if "git-repository-url" in html_sec:
        repo_url = html_sec["git-repository-url"]
        if not isinstance(repo_url, str) or not repo_url.startswith(
            GITHUB_ZEROCOPY_PREFIX
        ):
            errors.append(
                f"book/book.toml: `[output.html].git-repository-url` "
                f"must start with {GITHUB_ZEROCOPY_PREFIX!r}"
            )
    for theme_key in ("default-theme", "preferred-dark-theme"):
        if theme_key in html_sec and html_sec[theme_key] not in ALLOWED_MDBOOK_THEMES:
            errors.append(
                f"book/book.toml: `[output.html].{theme_key}` must be a built-in "
                f"mdBook theme ({sorted(ALLOWED_MDBOOK_THEMES)}), got "
                f"{html_sec[theme_key]!r}"
            )
    if "no-section-label" in html_sec and not isinstance(
        html_sec["no-section-label"], bool
    ):
        errors.append(
            "book/book.toml: `[output.html].no-section-label` must be a boolean"
        )


def _check_book_file_allowlist(
    book_root: Path, anneal_root: Path, errors: list[str]
) -> None:
    """Enforce a closed, exact-case file-type allowlist across `anneal/book/`."""
    for path in sorted(book_root.rglob("*")):
        rel = path.relative_to(book_root)
        if any(part.startswith(".") for part in rel.parts):
            errors.append(
                f"{path.relative_to(anneal_root)}: hidden file or directory "
                f"is not allowed under anneal/book/"
            )
            continue
        if path.is_dir():
            if rel.parts[0] not in {"src", "examples"}:
                errors.append(
                    f"{path.relative_to(anneal_root)}: disallowed directory "
                    f"under anneal/book/ (only `src/` and `examples/` are permitted)"
                )
            continue
        if not path.is_file():
            continue
        if rel == Path("book.toml"):
            continue
        if rel.parts[0] == "src":
            if path.suffix != ".md":
                errors.append(
                    f"{path.relative_to(anneal_root)}: only lowercase `.md` files "
                    f"are permitted under anneal/book/src/"
                )
        elif rel.parts[0] == "examples":
            if (
                path.name.lower() in {"cargo.toml", "cargo.lock"}
                or path.suffix not in ALLOWED_EXAMPLE_SUFFIXES
            ):
                errors.append(
                    f"{path.relative_to(anneal_root)}: disallowed file inside "
                    f"anneal/book/examples/ (allowed lowercase suffixes: "
                    f"{sorted(ALLOWED_EXAMPLE_SUFFIXES)})"
                )
        else:
            errors.append(
                f"{path.relative_to(anneal_root)}: unexpected top-level file "
                f"inside anneal/book/"
            )


def _strip_inline_code_spans(prose_text: str) -> str:
    """Strip single-line CommonMark inline code spans from Markdown prose.

    Scanning strictly per line (and skipping 4-space or tab-indented lines)
    prevents an indented fence or unclosed backtick on one line from masking
    raw HTML blocks (such as `<script>`) on subsequent lines.
    """
    out_lines: list[str] = []
    for line in prose_text.splitlines():
        if line.startswith(("    ", "\t")):
            out_lines.append(line)
            continue
        # Collect unescaped backtick runs `(start, end, length)` on this line.
        runs: list[tuple[int, int, int]] = []
        i = 0
        n = len(line)
        while i < n:
            if line[i] == "`":
                bs = 0
                b = i - 1
                while b >= 0 and line[b] == "\\":
                    bs += 1
                    b -= 1
                j = i
                while j < n and line[j] == "`":
                    j += 1
                if bs % 2 == 0:
                    runs.append((i, j, j - i))
                i = j
            else:
                i += 1
        # Map each backtick run length to its positions for O(n) matching.
        by_len: dict[int, list[int]] = {}
        for idx, (_s, _e, length) in enumerate(runs):
            by_len.setdefault(length, []).append(idx)

        out: list[str] = []
        cursor = 0
        run_idx = 0
        while run_idx < len(runs):
            start, end, length = runs[run_idx]
            positions = by_len[length]
            # Find the next run of the exact same length after `run_idx`.
            next_match = None
            for pos in positions:
                if pos > run_idx:
                    next_match = pos
                    break
            if next_match is not None:
                close_end = runs[next_match][1]
                out.append(line[cursor:start])
                out.append(" " * (close_end - start))
                cursor = close_end
                run_idx = next_match + 1
            else:
                run_idx += 1
        out.append(line[cursor:])
        out_lines.append("".join(out))
    return "\n".join(out_lines)


def _check_prose_for_raw_html(
    prose_text: str, rel_to_anneal: Path, errors: list[str]
) -> None:
    """Reject raw HTML tags (outside `<!-- ... -->` comments) and unsafe schemes."""
    without_comments = HTML_COMMENT_RE.sub(
        lambda m: "\n" * m.group(0).count("\n"), prose_text
    )
    without_code = _strip_inline_code_spans(without_comments)
    if RAW_HTML_TAG_RE.search(without_code):
        errors.append(
            f"{rel_to_anneal}: raw HTML tags are forbidden in book Markdown "
            f"(only `<!-- ... -->` comments are permitted)"
        )
    if UNSAFE_SCHEME_RE.search(without_code):
        errors.append(
            f"{rel_to_anneal}: disallowed script/data URL scheme in book Markdown"
        )


def _collect_verified_snippet_texts(anneal_root: Path) -> set[str]:
    """Return expected rendered code-block texts for all verified book fences.

    Reconstructs each checked, non-`conceptual` fence in `book/src/**/*.md` by
    evaluating its `{{#include ...}}` and `{{#rustdoc_include ...}}` directives
    through `_render_include_slice` against verified `case.json` artifacts so
    ranged, anchored, `rustdoc_include`, and multi-include fences match
    mdBook's exact rendered output.
    """
    book_root = anneal_root / "book"
    src_root = book_root / "src"
    examples_root = book_root / "examples"
    cases, errors = _load_example_cases(examples_root)
    if errors or not src_root.is_dir():
        return set()
    verified_artifacts: set[Path] = set()
    for _case_file, _data, artifacts in cases:
        verified_artifacts.update(artifacts)

    texts: set[str] = set()
    for md_file in sorted(src_root.rglob("*.md")):
        if md_file.is_symlink() or not md_file.is_file():
            continue
        text = md_file.read_text(encoding="utf-8")
        for fence in parse_fences(text):
            if fence.get("unclosed"):
                continue
            tokens = _parse_fence_tokens(fence["info"])
            matched_langs = sorted(tokens & CHECKED_FENCE_LANGS)
            if not matched_langs or "conceptual" in tokens:
                continue
            lang = "rust" if (tokens & {"rust", "rs"}) else matched_langs[0]
            rendered_lines: list[str] = []
            valid_fence = True
            has_directive = False
            for _ln, body_line in fence["body"]:
                stripped = body_line.strip()
                if not stripped:
                    rendered_lines.append("")
                    continue
                strict = STRICT_INCLUDE_RE.match(stripped)
                if not strict:
                    valid_fence = False
                    break
                has_directive = True
                kind = strict.group("kind")
                spec = strict.group("spec")
                try:
                    rel_target, anchor, line_range = _parse_include_spec(spec)
                except DocError:
                    valid_fence = False
                    break
                raw_target = md_file.parent / rel_target
                if _has_symlink_component(raw_target, book_root):
                    valid_fence = False
                    break
                target = raw_target.resolve()
                if (
                    not _is_within(target, examples_root)
                    or not target.is_file()
                    or target not in verified_artifacts
                ):
                    valid_fence = False
                    break
                target_text = target.read_text(encoding="utf-8")
                if anchor is not None:
                    anchors, anchor_errors = _extract_anchors(target_text)
                    if anchor_errors or anchor not in anchors:
                        valid_fence = False
                        break
                elif line_range is not None:
                    total_lines = len(target_text.splitlines())
                    start_opt, end_opt = line_range
                    start = 1 if start_opt is None else start_opt
                    end = total_lines if end_opt is None else end_opt
                    if start < 1 or end > total_lines or start > end:
                        valid_fence = False
                        break
                rendered_lines.append(
                    _render_include_slice(
                        target_text, kind, anchor, line_range, lang=lang
                    )
                )
            if valid_fence and has_directive:
                snippet = "\n".join(rendered_lines).strip()
                if snippet:
                    texts.add(snippet)
    return texts


def _collect_verified_rust_boring_lines(anneal_root: Path) -> set[str]:
    """Return allowed `<span class="boring">` lines for verified Rust blocks."""
    examples_root = anneal_root / "book/examples"
    cases, errors = _load_example_cases(examples_root)
    allowed = set(RUST_PLAYGROUND_BOILERPLATE_LINES)
    if errors:
        return allowed
    for _case_file, _data, artifacts in cases:
        for artifact in artifacts:
            if not artifact.is_file():
                continue
            for line in artifact.read_text(encoding="utf-8").splitlines():
                allowed.add(line)
                stripped = line.lstrip()
                if stripped == "#":
                    allowed.add(line[: len(line) - len(stripped)])
                elif stripped.startswith("# "):
                    indent = line[: len(line) - len(stripped)]
                    allowed.add(indent + stripped[2:])
    return allowed


def check_sources(anneal_root: Path) -> list[str]:
    """Validate book Markdown sources, fences, directives, examples, and governing docs."""
    errors: list[str] = []
    book_root = anneal_root / "book"
    src_root = book_root / "src"
    examples_root = book_root / "examples"

    _ensure_no_symlinks_under(book_root, "book", errors)
    if errors:
        return errors

    _check_book_toml(book_root, anneal_root, errors)
    _check_book_file_allowlist(book_root, anneal_root, errors)

    cases, case_errors = _load_example_cases(examples_root)
    errors.extend(case_errors)
    all_verified_artifacts: set[Path] = set()
    for _case_file, _data, artifacts in cases:
        all_verified_artifacts.update(artifacts)

    included_paths: set[Path] = set()
    rust_included_ranges: dict[Path, list[tuple[int, int, str]]] = {}

    md_files = sorted(src_root.rglob("*.md"))
    if not md_files:
        errors.append(f"no Markdown files found under {src_root}")
        return errors

    for md_file in md_files:
        rel_to_src = md_file.relative_to(src_root)
        rel_to_anneal = md_file.relative_to(anneal_root)
        top_section = rel_to_src.parts[0] if len(rel_to_src.parts) > 1 else ""
        text = md_file.read_text(encoding="utf-8")

        # 1. Check fenced code blocks against the default-deny policy.
        fences = parse_fences(text)
        checked_fence_lines: dict[int, str] = {}
        all_fence_lines: set[int] = set()
        for fence in fences:
            for ln in range(fence["start_line"], fence["end_line"] + 1):
                all_fence_lines.add(ln)
            if fence.get("unclosed"):
                errors.append(
                    f"{rel_to_anneal}:{fence['start_line']}: unclosed fenced code block"
                )
                continue
            tokens = _parse_fence_tokens(fence["info"])
            matched_langs = sorted(tokens & CHECKED_FENCE_LANGS)
            if not matched_langs:
                continue
            if "conceptual" in tokens:
                if top_section in STRICT_CHAPTER_DIRS:
                    errors.append(
                        f"{rel_to_anneal}:{fence['start_line']}: "
                        f"`conceptual` code fence is forbidden under `{top_section}/`"
                    )
                continue
            non_empty = [
                (ln, body_line.strip())
                for ln, body_line in fence["body"]
                if body_line.strip()
            ]
            if not non_empty or not all(
                STRICT_INCLUDE_RE.match(body_line) for _, body_line in non_empty
            ):
                errors.append(
                    f"{rel_to_anneal}:{fence['start_line']}: "
                    f"raw `{matched_langs[0]}` code fence is not allowed; include a "
                    f"verified artifact from book/examples/ via `{{{{#include ...}}}}`"
                )
            else:
                fence_lang = "rust" if (tokens & {"rust", "rs"}) else matched_langs[0]
                for ln, _ in non_empty:
                    checked_fence_lines[ln] = fence_lang

        # 2. Check all non-fence prose as a whole (catches multi-line raw HTML
        #    tags, attributes, and local Markdown images) and reject any stray
        #    unparsed fence lines.
        lines = text.splitlines()
        prose_only_lines = [
            "" if lineno in all_fence_lines else line
            for lineno, line in enumerate(lines, start=1)
        ]
        prose_text = "\n".join(prose_only_lines)
        _check_prose_for_raw_html(prose_text, rel_to_anneal, errors)
        _check_prose_for_local_images(prose_text, rel_to_anneal, errors)

        for lineno, line in enumerate(lines, start=1):
            if lineno not in all_fence_lines and STRAY_FENCE_RE.match(line):
                errors.append(
                    f"{rel_to_anneal}:{lineno}: nested or indented code fence "
                    f"marker outside a top-level fence is not allowed"
                )

            for match in ANY_DIRECTIVE_RE.finditer(line):
                full_directive = match.group(0)
                strict = STRICT_INCLUDE_RE.match(full_directive)
                if not strict:
                    errors.append(
                        f"{rel_to_anneal}:{lineno}: disallowed or malformed mdBook "
                        f"directive `{full_directive}`; only single-token "
                        f"`{{{{#include <spec>}}}}` and `{{{{#rustdoc_include <spec>}}}}` "
                        f"are permitted"
                    )
                    continue
                if lineno not in checked_fence_lines:
                    errors.append(
                        f"{rel_to_anneal}:{lineno}: `{{{{#{strict.group('kind')} ...}}}}` "
                        f"is only permitted inside a checked code fence"
                    )
                    continue

                kind = strict.group("kind")
                spec = strict.group("spec")
                try:
                    rel_target, anchor, line_range = _parse_include_spec(spec)
                except DocError as exc:
                    errors.append(f"{rel_to_anneal}:{lineno}: {exc}")
                    continue

                raw_target = md_file.parent / rel_target
                if _has_symlink_component(raw_target, book_root):
                    errors.append(
                        f"{rel_to_anneal}:{lineno}: include target traverses a "
                        f"symlink: {rel_target}"
                    )
                    continue
                target = raw_target.resolve()
                if not _is_within(target, examples_root):
                    errors.append(
                        f"{rel_to_anneal}:{lineno}: include target must reside "
                        f"under anneal/book/examples/: {rel_target}"
                    )
                    continue
                if not target.is_file():
                    errors.append(
                        f"{rel_to_anneal}:{lineno}: include target is missing or "
                        f"not a regular file: {rel_target}"
                    )
                    continue
                if target not in all_verified_artifacts:
                    errors.append(
                        f"{rel_to_anneal}:{lineno}: include target is not a "
                        f"registered verified artifact of any case.json: {rel_target}"
                    )
                    continue

                included_paths.add(target)
                target_text = target.read_text(encoding="utf-8")
                target_lines = target_text.splitlines()
                fence_lang = checked_fence_lines[lineno]

                if anchor is not None:
                    anchors, anchor_errors = _extract_anchors(target_text)
                    for err in anchor_errors:
                        errors.append(f"{target.relative_to(anneal_root)}: {err}")
                    if anchor not in anchors:
                        errors.append(
                            f"{rel_to_anneal}:{lineno}: missing anchor `{anchor}` "
                            f"in {rel_target}"
                        )
                        continue
                    start_line, end_line = anchors[anchor]
                    rendered_slice = _render_include_slice(
                        target_text,
                        kind,
                        anchor,
                        None,
                        lang=fence_lang,
                    )
                    if not rendered_slice.strip():
                        errors.append(
                            f"{rel_to_anneal}:{lineno}: anchor `{anchor}` in "
                            f"{rel_target} is empty"
                        )
                        continue
                    if target.suffix == ".rs":
                        rust_included_ranges.setdefault(target, []).append(
                            (start_line + 1, end_line - 1, kind)
                        )
                elif line_range is not None:
                    start_opt, end_opt = line_range
                    start = 1 if start_opt is None else start_opt
                    end = len(target_lines) if end_opt is None else end_opt
                    if start < 1 or end > len(target_lines) or start > end:
                        errors.append(
                            f"{rel_to_anneal}:{lineno}: invalid line range "
                            f"`{start}:{end}` for {rel_target} ({len(target_lines)} lines)"
                        )
                        continue
                    rendered_slice = _render_include_slice(
                        target_text,
                        kind,
                        None,
                        line_range,
                        lang=fence_lang,
                    )
                    if not rendered_slice.strip():
                        errors.append(
                            f"{rel_to_anneal}:{lineno}: line range `{start}:{end}` "
                            f"in {rel_target} is empty"
                        )
                        continue
                    if target.suffix == ".rs":
                        rust_included_ranges.setdefault(target, []).append(
                            (start, end, kind)
                        )
                else:
                    rendered_slice = _render_include_slice(
                        target_text,
                        kind,
                        None,
                        None,
                        lang=fence_lang,
                    )
                    if not rendered_slice.strip():
                        errors.append(
                            f"{rel_to_anneal}:{lineno}: included file is empty: "
                            f"{rel_target}"
                        )
                    if target.suffix == ".rs":
                        rust_included_ranges.setdefault(target, []).append(
                            (1, len(target_lines), kind)
                        )

    # 3. Verify no hidden load-bearing Rust lines in anchored `.rs` includes.
    for rs_path, ranges in sorted(rust_included_ranges.items()):
        _validate_rust_anchor_coverage(rs_path, ranges, errors)

    # 4. Verify every auto-discovered example case has at least one verified
    #    artifact referenced by the book.
    for case_file, _data, artifacts in cases:
        if artifacts and not (artifacts & included_paths):
            errors.append(
                f"{case_file.relative_to(anneal_root)}: example case's verified "
                f"artifact is not included in any book chapter"
            )

    # 5. Verify staged governing docs exist and have no unmapped relative links
    #    or duplicated copies checked into `book/src/foundations/`.
    staged_targets = {dest_rel for _, dest_rel in GOVERNING_DOCS}
    for src_name, dest_rel in GOVERNING_DOCS:
        src_path = anneal_root / src_name
        if not src_path.is_file():
            errors.append(f"missing canonical governing doc: {src_name}")
            continue
        dup_path = src_root / dest_rel
        if dup_path.exists():
            errors.append(
                f"{dup_path.relative_to(anneal_root)}: do not check in a "
                f"duplicate copy of {src_name}; it is staged automatically at "
                f"build time"
            )
        _, link_errors = _rewrite_governing_doc(src_path, dest_rel, anneal_root)
        errors.extend(link_errors)

    # 6. Verify every chapter target listed in `book/src/SUMMARY.md` exists or
    #    is one of the staged governing docs.
    summary_path = src_root / "SUMMARY.md"
    if not summary_path.is_file():
        errors.append(
            f"missing book summary: {summary_path.relative_to(anneal_root)}"
        )
    else:
        summary_text = summary_path.read_text(encoding="utf-8")
        for match in MD_LINK_RE.finditer(summary_text):
            target_rel = match.group("target").split("#", 1)[0]
            if not target_rel or target_rel.startswith(("http://", "https://")):
                continue
            if target_rel in staged_targets:
                continue
            candidate = (src_root / target_rel).resolve()
            if (
                not _is_within(candidate, src_root)
                or not candidate.is_file()
                or candidate.suffix != ".md"
            ):
                errors.append(
                    f"book/src/SUMMARY.md: missing or invalid chapter target `{target_rel}`"
                )

    return errors


def _run_cli_case(
    case_file: Path,
    data: dict,
    cargo_anneal: Path,
    bless: bool,
) -> list[str]:
    """Execute one allowlisted CLI example case against `cargo_anneal`."""
    errors: list[str] = []
    with tempfile.TemporaryDirectory(prefix="anneal-doc-cli-") as tmp_str:
        tmp = Path(tmp_str)
        work_dir = tmp / "work"
        work_dir.mkdir()
        work_resolved = work_dir.resolve()

        setup_files = data.get("setup_files") or {}
        if not isinstance(setup_files, dict):
            return [f"{case_file}: `setup_files` must be an object"]
        for rel_name, content in setup_files.items():
            if not isinstance(rel_name, str) or not isinstance(content, str):
                return [
                    f"{case_file}: `setup_files` entries must map strings to strings"
                ]
            dest = (work_dir / rel_name).resolve()
            if dest == work_resolved or not _is_within(dest, work_dir):
                return [
                    f"{case_file}: setup_files path invalid or escapes workdir: {rel_name}"
                ]
            dest.parent.mkdir(parents=True, exist_ok=True)
            dest.write_text(content, encoding="utf-8")

        env = {
            "PATH": os.environ.get("PATH", ""),
            "HOME": str(tmp / "home"),
            "TERM": "dumb",
            "NO_COLOR": "1",
        }
        (tmp / "home").mkdir()
        if data.get("fresh_cache_env", False):
            cache_dir = tmp / "cache"
            cache_dir.mkdir()
            env["XDG_CACHE_HOME"] = str(cache_dir)

        proc = subprocess.run(
            [str(cargo_anneal), *data["argv"]],
            cwd=work_dir,
            env=env,
            capture_output=True,
            text=True,
            timeout=60,
            check=False,
        )

        expected_exit = data["expect_exit"]
        if proc.returncode != expected_exit:
            errors.append(
                f"{case_file}: expected exit code {expected_exit}, got "
                f"{proc.returncode}\nstdout:\n{proc.stdout}\nstderr:\n{proc.stderr}"
            )
            return errors

        combined_output = proc.stdout + proc.stderr
        if data.get("normalize_tempdir", False):
            combined_output = (
                combined_output.replace(str(work_resolved), "<WORKDIR>")
                .replace(str(work_dir), "<WORKDIR>")
                .replace(str(tmp.resolve()), "<TMPDIR>")
                .replace(str(tmp), "<TMPDIR>")
            )

        actual_transcript = f"{data['display_command']}\n{combined_output}"
        if not actual_transcript.endswith("\n"):
            actual_transcript += "\n"

        transcript_path = case_file.parent / data["transcript"]
        if bless:
            transcript_path.write_text(actual_transcript, encoding="utf-8")
            return []

        expected_transcript = transcript_path.read_text(encoding="utf-8")
        if actual_transcript != expected_transcript:
            diff = "".join(
                difflib.unified_diff(
                    expected_transcript.splitlines(keepends=True),
                    actual_transcript.splitlines(keepends=True),
                    fromfile=str(transcript_path),
                    tofile="<live cargo-anneal output>",
                )
            )
            errors.append(
                f"{case_file}: transcript mismatch (re-run with `--bless` if "
                f"this CLI change is intentional):\n{diff}"
            )
    return errors


def _normalize_output_paths(
    text: str, work_dir: Path, work_resolved: Path, tmp_dir: Path
) -> str:
    """Replace temporary workspace and staging paths with deterministic tags."""
    out = (
        text.replace(str(work_resolved), "<WORKDIR>")
        .replace(str(work_dir), "<WORKDIR>")
        .replace(str(tmp_dir.resolve()), "<TMPDIR>")
        .replace(str(tmp_dir), "<TMPDIR>")
    )
    out = re.sub(
        r"/tmp/anneal-verification-[A-Za-z0-9]+", "<ANNEAL_STAGE_DIR>", out
    )
    return out


def _assert_and_summarize_receipt(
    stdout: str,
    stderr: str,
    expect_receipt: dict,
    case_file: Path,
    step_idx: int,
    work_dir: Path,
    work_resolved: Path,
    tmp_dir: Path,
) -> tuple[dict | None, list[str]]:
    """Validate live Receipt v3 invariants and build a deterministic summary dict."""
    errors: list[str] = []
    try:
        receipt = json.loads(stdout)
    except json.JSONDecodeError as exc:
        return None, [
            f"{case_file}: step[{step_idx}] invalid Receipt JSON on stdout: "
            f"{exc}\nstdout={stdout!r}\nstderr={stderr!r}"
        ]
    if not isinstance(receipt, dict):
        return None, [f"{case_file}: step[{step_idx}] receipt is not a JSON object"]

    expected_sv = expect_receipt.get("schema_version", 3)
    if receipt.get("schema_version") != expected_sv:
        errors.append(
            f"{case_file}: step[{step_idx}] wrong schema_version: "
            f"{receipt.get('schema_version')!r}"
        )
    if receipt.get("status") != expect_receipt["status"]:
        errors.append(
            f"{case_file}: step[{step_idx}] expected status "
            f"{expect_receipt['status']!r}, got {receipt.get('status')!r} "
            f"(hint={receipt.get('hint')!r})"
        )
    if "tainted" in receipt or receipt.get("taint_reasons") != []:
        errors.append(
            f"{case_file}: step[{step_idx}] unexpected taint in receipt: "
            f"taint_reasons={receipt.get('taint_reasons')!r}"
        )
    req_sbx = expect_receipt.get("elaboration_sandbox_in", ALLOWED_SANDBOXES)
    allowed_sbx = set(req_sbx) & ALLOWED_SANDBOXES
    if receipt.get("elaboration_sandbox") not in allowed_sbx:
        errors.append(
            f"{case_file}: step[{step_idx}] unexpected sandbox "
            f"{receipt.get('elaboration_sandbox')!r}"
        )
    for key in (
        "experimental",
        "phase",
        "spec_warnings",
        "scope",
        "assumptions",
        "deferred",
    ):
        if key in expect_receipt and receipt.get(key) != expect_receipt[key]:
            errors.append(
                f"{case_file}: step[{step_idx}] mismatch for `{key}`: "
                f"expected {expect_receipt[key]!r}, got {receipt.get(key)!r}"
            )
    if "selected_paths" in expect_receipt:
        actual_paths = [item.get("path") for item in (receipt.get("selected") or [])]
        if actual_paths != expect_receipt["selected_paths"]:
            errors.append(
                f"{case_file}: step[{step_idx}] mismatch for `selected` paths: "
                f"expected {expect_receipt['selected_paths']!r}, got {actual_paths!r}"
            )
    if "hint_contains" in expect_receipt:
        hint = receipt.get("hint") or ""
        if expect_receipt["hint_contains"] not in hint:
            errors.append(
                f"{case_file}: step[{step_idx}] missing hint substring "
                f"{expect_receipt['hint_contains']!r} in {hint!r}"
            )
    if "detail_contains" in expect_receipt:
        detail = receipt.get("detail") or ""
        if expect_receipt["detail_contains"] not in detail:
            errors.append(
                f"{case_file}: step[{step_idx}] missing detail substring "
                f"{expect_receipt['detail_contains']!r} in {detail!r}"
            )
    if "stderr_contains" in expect_receipt:
        if expect_receipt["stderr_contains"] not in stderr:
            errors.append(
                f"{case_file}: step[{step_idx}] missing stderr substring "
                f"{expect_receipt['stderr_contains']!r} in {stderr!r}"
            )

    summary: dict = {
        "schema_version": receipt.get("schema_version"),
        "status": receipt.get("status"),
    }
    sbx_summary = "<linux_bwrap | linux_landlock_seccomp>"
    actual_status = receipt.get("status")
    if isinstance(actual_status, str) and actual_status in VERIFIED_TUTOR_STATUSES:
        for field in (
            "experimental",
            "elaboration_sandbox",
            "taint_reasons",
            "spec_warnings",
            "scope",
            "selected",
            "assumptions",
            "deferred",
        ):
            if field == "elaboration_sandbox" and field in receipt:
                summary[field] = sbx_summary
            elif field in receipt:
                summary[field] = receipt[field]
    else:
        for field in ("phase", "elaboration_sandbox", "taint_reasons"):
            if field == "elaboration_sandbox" and field in receipt:
                summary[field] = sbx_summary
            elif field in receipt:
                summary[field] = receipt[field]
        if receipt.get("hint"):
            summary["hint"] = _normalize_output_paths(
                str(receipt["hint"]), work_dir, work_resolved, tmp_dir
            )
    return summary, errors


def _compare_or_bless(
    target_path: Path, actual_text: str, bless: bool, label: str
) -> list[str]:
    """Verify `actual_text` matches `target_path` or overwrite when `bless=True`."""
    if bless:
        target_path.write_text(actual_text, encoding="utf-8")
        return []
    expected_text = target_path.read_text(encoding="utf-8")
    if actual_text != expected_text:
        diff = "".join(
            difflib.unified_diff(
                expected_text.splitlines(keepends=True),
                actual_text.splitlines(keepends=True),
                fromfile=str(target_path),
                tofile=f"<live {label}>",
            )
        )
        return [
            f"{target_path}: output mismatch (re-run with `--bless` if "
            f"intentional):\n{diff}"
        ]
    return []


def _inject_tutor_toolchain(
    argv: list[str],
    capture: Path | None,
    sysroot: Path | None,
    lean: Path | None,
) -> list[str]:
    """Inject explicit `--capture`/`--sysroot`/`--lean` into `verify` invocations."""
    if len(argv) >= 2 and argv[1] == "verify" and capture and sysroot and lean:
        return [
            argv[0],
            argv[1],
            "--capture",
            str(capture),
            "--sysroot",
            str(sysroot),
            "--lean",
            str(lean),
            *argv[2:],
        ]
    return list(argv)


def _run_tutor_case(
    case_file: Path,
    data: dict,
    cargo_anneal: Path,
    *,
    session_cache: Path,
    host_xdg_cache: str | None,
    capture: Path | None,
    sysroot: Path | None,
    lean: Path | None,
    bless: bool,
) -> list[str]:
    """Execute a multi-step isolated `"kind": "tutor"` verification scenario."""
    errors: list[str] = []
    with tempfile.TemporaryDirectory(prefix="anneal-doc-tutor-") as tmp_str:
        tmp = Path(tmp_str)
        work_dir = tmp / "work"
        work_dir.mkdir()
        work_resolved = work_dir.resolve()
        home_dir = tmp / "home"
        home_dir.mkdir()

        for art in data["artifacts"]:
            if art["role"] != "input":
                continue
            src = case_file.parent / art["file"]
            raw_dest = work_dir / art["stage_as"]
            if _has_symlink_component(raw_dest, work_dir):
                return [f"{case_file}: stage_as traverses symlink: {art['stage_as']!r}"]
            dest = raw_dest.resolve()
            if not _is_within(dest, work_dir):
                return [f"{case_file}: invalid stage_as {art['stage_as']!r}"]
            dest.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(src, dest)

        env = {
            "PATH": os.environ.get("PATH", ""),
            "HOME": str(home_dir),
            "TMPDIR": str(tmp),
            "TERM": "dumb",
            "NO_COLOR": "1",
        }
        if any((capture, sysroot, lean)) and not all((capture, sysroot, lean)):
            return [
                f"{case_file}: partial explicit toolchain overrides; pass all "
                f"three of capture/sysroot/lean or none"
            ]
        has_installed_bundle = bool(
            host_xdg_cache
            and (Path(host_xdg_cache) / "anneal" / "toolchain").is_dir()
        )
        use_explicit_tools = bool(capture and sysroot and lean)
        if use_explicit_tools:
            env["XDG_CACHE_HOME"] = str(session_cache)
        elif has_installed_bundle and host_xdg_cache:
            env["XDG_CACHE_HOME"] = host_xdg_cache
        else:
            env["XDG_CACHE_HOME"] = str(session_cache)

        checked_generated: set[str] = set()

        def _check_generated(targets: set[str] | None = None) -> list[str]:
            gen_errs: list[str] = []
            for art in data["artifacts"]:
                if art["role"] != "generated":
                    continue
                wd_rel = art["workdir_path"]
                if wd_rel in checked_generated:
                    continue
                if targets is not None and wd_rel not in targets:
                    continue
                raw_gen = work_dir / wd_rel
                if _has_symlink_component(raw_gen, work_dir):
                    gen_errs.append(
                        f"{case_file}: generated file traverses symlink: {wd_rel!r}"
                    )
                    checked_generated.add(wd_rel)
                    continue
                gen_path = raw_gen.resolve()
                if not _is_within(gen_path, work_dir) or not gen_path.is_file():
                    gen_errs.append(
                        f"{case_file}: missing generated workdir file {wd_rel!r}"
                    )
                    checked_generated.add(wd_rel)
                    continue
                gen_text = gen_path.read_text(encoding="utf-8")
                gen_errs.extend(
                    _compare_or_bless(
                        case_file.parent / art["file"],
                        gen_text,
                        bless,
                        f"generated {wd_rel}",
                    )
                )
                checked_generated.add(wd_rel)
            return gen_errs

        generated_wd_paths = {
            art["workdir_path"]
            for art in data["artifacts"]
            if art["role"] == "generated"
        }

        for step_idx, step in enumerate(data["steps"]):
            copy_entries = step.get("copy_files") or []
            overwritten_generated = {
                cp_entry["dest"]
                for cp_entry in copy_entries
                if cp_entry["dest"] in generated_wd_paths
                and cp_entry["dest"] not in checked_generated
            }
            if overwritten_generated:
                snap_errs = _check_generated(overwritten_generated)
                if snap_errs:
                    return snap_errs
            for cp_entry in copy_entries:
                src = case_file.parent / cp_entry["src"]
                raw_dest = work_dir / cp_entry["dest"]
                if _has_symlink_component(raw_dest, work_dir):
                    return [
                        f"{case_file}: copy dest traverses symlink: "
                        f"{cp_entry['dest']!r}"
                    ]
                dest = raw_dest.resolve()
                if not _is_within(dest, work_dir):
                    return [f"{case_file}: invalid copy dest {cp_entry['dest']!r}"]
                dest.parent.mkdir(parents=True, exist_ok=True)
                shutil.copyfile(src, dest)

            argv = step["argv"]
            if (
                len(argv) >= 2
                and argv[1] == "verify"
                and not (use_explicit_tools or has_installed_bundle)
            ):
                return [
                    f"{case_file}: step[{step_idx}] `verify` requires --capture/"
                    f"--sysroot/--lean (or installed XDG_CACHE_HOME toolchain)"
                ]

            exec_argv = (
                _inject_tutor_toolchain(argv, capture, sysroot, lean)
                if use_explicit_tools
                else list(argv)
            )
            try:
                proc = subprocess.run(
                    [str(cargo_anneal), *exec_argv],
                    cwd=work_dir,
                    env=env,
                    capture_output=True,
                    text=True,
                    timeout=240,
                    check=False,
                )
            except subprocess.TimeoutExpired as exc:
                return [
                    f"{case_file}: step[{step_idx}] timed out after "
                    f"{exc.timeout}s"
                ]
            if proc.returncode != step["expect_exit"]:
                errors.append(
                    f"{case_file}: step[{step_idx}] expected exit "
                    f"{step['expect_exit']}, got {proc.returncode}\n"
                    f"stdout:\n{proc.stdout}\nstderr:\n{proc.stderr}"
                )
                return errors

            summary = None
            if "expect_receipt" in step:
                summary, rec_errs = _assert_and_summarize_receipt(
                    proc.stdout,
                    proc.stderr,
                    step["expect_receipt"],
                    case_file,
                    step_idx,
                    work_dir,
                    work_resolved,
                    tmp,
                )
                errors.extend(rec_errs)
                if rec_errs:
                    return errors

            if "receipt_excerpt" in step:
                if summary is None:
                    errors.append(
                        f"{case_file}: step[{step_idx}] `receipt_excerpt` requires "
                        f"`expect_receipt`"
                    )
                else:
                    excerpt_text = json.dumps(summary, indent=2) + "\n"
                    errors.extend(
                        _compare_or_bless(
                            case_file.parent / step["receipt_excerpt"],
                            excerpt_text,
                            bless,
                            "receipt_excerpt",
                        )
                    )

            if "transcript" in step:
                mode = step.get("output_mode") or (
                    "receipt_summary" if summary is not None else "full"
                )
                if mode == "receipt_summary":
                    if summary is None:
                        errors.append(
                            f"{case_file}: step[{step_idx}] `receipt_summary` "
                            f"requires `expect_receipt`"
                        )
                        return errors
                    body = json.dumps(summary, indent=2) + "\n"
                else:
                    body = _normalize_output_paths(
                        proc.stdout + proc.stderr, work_dir, work_resolved, tmp
                    )
                actual_transcript = f"{step['display_command']}\n{body}"
                if not actual_transcript.endswith("\n"):
                    actual_transcript += "\n"
                errors.extend(
                    _compare_or_bless(
                        case_file.parent / step["transcript"],
                        actual_transcript,
                        bless,
                        "tutor transcript",
                    )
                )

        errors.extend(_check_generated(None))
    return errors


def verify_examples(
    anneal_root: Path,
    cargo_anneal: Path,
    *,
    bless: bool = False,
    capture: Path | None = None,
    sysroot: Path | None = None,
    lean: Path | None = None,
) -> list[str]:
    """Run all auto-discovered doc examples against `cargo_anneal`."""
    cargo_anneal_resolved = cargo_anneal.resolve()
    if not cargo_anneal_resolved.is_file():
        return [f"cargo-anneal binary does not exist: {cargo_anneal}"]

    examples_root = anneal_root / "book/examples"
    cases, errors = _load_example_cases(examples_root)
    if errors:
        return errors

    host_xdg_cache = os.environ.get("XDG_CACHE_HOME") or None
    with tempfile.TemporaryDirectory(prefix="anneal-doc-cache-") as cache_str:
        session_cache = Path(cache_str)
        for case_file, data, _artifacts in cases:
            if data["kind"] == "cli":
                errors.extend(
                    _run_cli_case(
                        case_file, data, cargo_anneal_resolved, bless=bless
                    )
                )
            elif data["kind"] == "tutor":
                errors.extend(
                    _run_tutor_case(
                        case_file,
                        data,
                        cargo_anneal_resolved,
                        session_cache=session_cache,
                        host_xdg_cache=host_xdg_cache,
                        capture=capture,
                        sysroot=sysroot,
                        lean=lean,
                        bless=bless,
                    )
                )
            else:
                errors.append(
                    f"{case_file}: unsupported example kind: {data['kind']!r}"
                )
    return errors


def _sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(65536), b""):
            digest.update(chunk)
    return digest.hexdigest()


def _download_with_retries(url: str, dest: Path, attempts: int = 3) -> None:
    """Download `url` into `dest` with bounded retries on transient errors."""
    last_exc: Exception | None = None
    for attempt in range(1, attempts + 1):
        try:
            with urllib.request.urlopen(url, timeout=60) as resp:
                dest.write_bytes(resp.read())
            return
        except (URLError, TimeoutError, http.client.HTTPException, OSError) as exc:
            last_exc = exc
            if attempt < attempts:
                time.sleep(attempt)
    raise DocError(
        f"failed to download {url} after {attempts} attempts: {last_exc}"
    )


def ensure_mdbook(
    cache_dir: Path,
    explicit_bin: Path | None = None,
    *,
    url: str = MDBOOK_URL,
    expected_tarball_sha256: str = MDBOOK_SHA256,
    expected_bin_sha256: str = MDBOOK_BIN_SHA256,
) -> Path:
    """Return a verified absolute `mdbook` binary, fetching the pinned release if needed."""
    if explicit_bin is not None:
        if explicit_bin.is_symlink():
            raise DocError(
                f"explicit --mdbook binary is missing or a symlink: {explicit_bin}"
            )
        resolved = explicit_bin.resolve()
        if not resolved.is_file():
            raise DocError(
                f"explicit --mdbook binary is missing or a symlink: {explicit_bin}"
            )
        return resolved

    if not url.startswith("https://"):
        raise DocError(f"mdBook URL must use https: {url}")

    cache_dir = cache_dir.resolve()
    cache_dir.mkdir(parents=True, exist_ok=True)
    install_dir = cache_dir / f"mdbook-v{MDBOOK_VERSION}-{MDBOOK_TARGET}"
    binary_path = install_dir / "mdbook"

    if (
        not binary_path.is_symlink()
        and binary_path.is_file()
        and _sha256_file(binary_path) == expected_bin_sha256
    ):
        return binary_path

    tarball = cache_dir / f"mdbook-v{MDBOOK_VERSION}-{MDBOOK_TARGET}.tar.gz"
    tmp_tarball = cache_dir / f".mdbook-{os.getpid()}.tar.gz.part"
    try:
        _download_with_retries(url, tmp_tarball)
        actual_sha = _sha256_file(tmp_tarball)
        if actual_sha != expected_tarball_sha256:
            raise DocError(
                f"mdBook tarball SHA-256 mismatch: expected "
                f"{expected_tarball_sha256}, got {actual_sha}"
            )
        tmp_tarball.replace(tarball)
    finally:
        if tmp_tarball.exists():
            tmp_tarball.unlink()

    if install_dir.exists():
        shutil.rmtree(install_dir)
    install_dir.mkdir(parents=True)

    with tarfile.open(tarball, "r:gz") as archive:
        members = archive.getmembers()
        if (
            len(members) != 1
            or members[0].name != "mdbook"
            or not members[0].isfile()
        ):
            raise DocError(
                f"unexpected mdBook tarball contents: {[m.name for m in members]}"
            )
        extracted = archive.extractfile(members[0])
        if extracted is None:
            raise DocError("failed to extract mdbook binary from tarball")
        binary_path.write_bytes(extracted.read())
    actual_bin_sha = _sha256_file(binary_path)
    if actual_bin_sha != expected_bin_sha256:
        binary_path.unlink(missing_ok=True)
        raise DocError(
            f"mdBook binary SHA-256 mismatch: expected {expected_bin_sha256}, "
            f"got {actual_bin_sha}"
        )
    binary_path.chmod(0o755)
    return binary_path


class _HtmlAuditor(HTMLParser):
    """Audit rendered HTML for link/media targets, `<main>` structure, XSS, and code blocks."""

    def __init__(self) -> None:
        super().__init__()
        self.ids: set[str] = set()
        self.hrefs: list[str] = []
        self.srcs: list[str] = []
        self.main_depth = 0
        self.main_open_count = 0
        self.main_close_count = 0
        self.pre_depth = 0
        self.in_checked_code = False
        self.current_code_lang = ""
        self.current_code_conceptual = False
        self.current_code_chunks: list[str] = []
        self.current_boring_chunks: list[str] = []
        self.code_span_is_boring: list[bool] = []
        self.boring_span_depth = 0
        self.checked_code_blocks: list[tuple[str, bool, str]] = []
        self.checked_code_boring_blocks: list[tuple[str, bool, str]] = []
        self.unsafe_findings: list[str] = []

    def handle_starttag(self, tag: str, attrs: list[tuple[str, str | None]]) -> None:
        tag_l = tag.lower()
        seen_attrs: set[str] = set()
        for raw_name, raw_val in attrs:
            attr_name = raw_name.lower()
            if attr_name in seen_attrs:
                self.unsafe_findings.append(
                    f"duplicate attribute `{attr_name}` on `<{tag_l}>`"
                )
            seen_attrs.add(attr_name)
            if attr_name == "id" and raw_val:
                self.ids.add(raw_val)
            if tag_l == "a" and attr_name == "name" and raw_val:
                self.ids.add(raw_val)
            if tag_l == "a" and attr_name == "href" and raw_val is not None:
                self.hrefs.append(raw_val)
            if self.main_depth > 0:
                if (
                    tag_l in MEDIA_SRC_TAGS
                    and attr_name == "src"
                    and raw_val is not None
                ):
                    self.srcs.append(raw_val)
                if attr_name.startswith("on"):
                    self.unsafe_findings.append(
                        f"disallowed event-handler attribute `{attr_name}` "
                        f"inside rendered `<main>`"
                    )
                if attr_name in URL_ATTRS and raw_val is not None:
                    val_stripped = raw_val.strip()
                    parsed = urlsplit(val_stripped)
                    scheme = parsed.scheme.lower()
                    if attr_name == "srcdoc" or (
                        scheme and scheme not in {"http", "https", "mailto"}
                    ):
                        self.unsafe_findings.append(
                            f"disallowed URL attribute `{attr_name}={raw_val!r}` "
                            f"inside rendered `<main>`"
                        )

        if tag_l == "main":
            self.main_open_count += 1
            self.main_depth += 1
            if self.main_open_count > 1:
                self.unsafe_findings.append("multiple `<main>` elements on page")
        elif self.main_depth > 0:
            if tag_l in DISALLOWED_MAIN_HTML_TAGS:
                self.unsafe_findings.append(
                    f"disallowed `<{tag_l}>` element inside rendered `<main>`"
                )
            if tag_l == "pre":
                self.pre_depth += 1
            elif tag_l == "code" and self.pre_depth > 0:
                classes: set[str] = set()
                for raw_name, raw_val in attrs:
                    if raw_name.lower() == "class" and raw_val:
                        classes.update(raw_val.split())
                for cls in sorted(classes):
                    if cls.startswith("language-"):
                        lang = cls[len("language-") :].lower()
                        if lang in CHECKED_FENCE_LANGS:
                            self.in_checked_code = True
                            self.current_code_lang = lang
                            self.current_code_conceptual = "conceptual" in classes
                            self.current_code_chunks = []
                            self.current_boring_chunks = []
                            self.code_span_is_boring = []
                            self.boring_span_depth = 0
                            break
            elif (
                tag_l == "span"
                and self.in_checked_code
                and self.current_code_lang in {"rust", "rs"}
            ):
                classes = set()
                for raw_name, raw_val in attrs:
                    if raw_name.lower() == "class" and raw_val:
                        classes.update(raw_val.split())
                is_boring = "boring" in classes
                self.code_span_is_boring.append(is_boring)
                if is_boring:
                    self.boring_span_depth += 1

    def handle_endtag(self, tag: str) -> None:
        tag_l = tag.lower()
        if (
            tag_l == "span"
            and self.in_checked_code
            and self.code_span_is_boring
        ):
            was_boring = self.code_span_is_boring.pop()
            if was_boring and self.boring_span_depth > 0:
                self.boring_span_depth -= 1
        elif tag_l == "code" and self.in_checked_code:
            self.checked_code_blocks.append(
                (
                    self.current_code_lang,
                    self.current_code_conceptual,
                    "".join(self.current_code_chunks).strip(),
                )
            )
            self.checked_code_boring_blocks.append(
                (
                    self.current_code_lang,
                    self.current_code_conceptual,
                    "".join(self.current_boring_chunks),
                )
            )
            self.in_checked_code = False
            self.current_code_lang = ""
            self.current_code_conceptual = False
            self.current_code_chunks = []
            self.current_boring_chunks = []
            self.code_span_is_boring = []
            self.boring_span_depth = 0
        elif tag_l == "pre" and self.pre_depth > 0:
            self.pre_depth -= 1
        elif tag_l == "main":
            self.main_close_count += 1
            if self.main_depth == 0:
                self.unsafe_findings.append("unmatched `</main>` tag on page")
            else:
                self.main_depth -= 1

    def handle_data(self, data: str) -> None:
        if self.in_checked_code:
            if self.boring_span_depth > 0:
                self.current_boring_chunks.append(data)
            else:
                self.current_code_chunks.append(data)


def check_built_html_links(
    html_root: Path,
    site_url_prefix: str = SITE_URL_PREFIX,
    *,
    verified_snippets: set[str] | None = None,
    verified_boring_lines: set[str] | None = None,
) -> list[str]:
    """Verify built HTML links, media `src` targets, `#fragments`, `<main>` hygiene, and code parity."""
    errors: list[str] = []
    html_files = sorted(html_root.rglob("*.html"))
    if not html_files:
        return [f"no HTML files generated under {html_root}"]

    page_ids: dict[Path, set[str]] = {}
    page_hrefs: dict[Path, list[str]] = {}
    page_srcs: dict[Path, list[str]] = {}
    root_resolved = html_root.resolve()
    skip_code_parity_pages = {
        "foundations/principles.html",
        "foundations/threat-model.html",
        "foundations/meta-principles.html",
        "print.html",
    }

    for html_file in html_files:
        auditor = _HtmlAuditor()
        auditor.feed(html_file.read_text(encoding="utf-8"))
        auditor.close()
        resolved = html_file.resolve()
        rel_page = resolved.relative_to(root_resolved)
        rel_posix = rel_page.as_posix()

        for finding in auditor.unsafe_findings:
            errors.append(f"{rel_page}: {finding}")
        # Require balanced `<main>...</main>` on every page that contains `<main>`
        # (mdBook `toc.html` is a sidebar iframe without `<main>`).
        if auditor.main_open_count != auditor.main_close_count or auditor.main_depth != 0:
            errors.append(
                f"{rel_page}: unbalanced `<main>` tags "
                f"(open={auditor.main_open_count}, close={auditor.main_close_count})"
            )

        # Structural defense against CommonMark parser desyncs: every rendered
        # `CHECKED_FENCE_LANGS` code block in authored chapters must match a
        # verified example artifact slice (and `conceptual` is forbidden in
        # strict chapters).
        if verified_snippets is not None and rel_posix not in skip_code_parity_pages:
            top_section = rel_page.parts[0] if len(rel_page.parts) > 1 else ""
            for (lang, is_conceptual, code_text), (
                _b_lang,
                _b_conc,
                boring_text,
            ) in zip(
                auditor.checked_code_blocks,
                auditor.checked_code_boring_blocks,
            ):
                if is_conceptual:
                    if top_section in STRICT_CHAPTER_DIRS:
                        errors.append(
                            f"{rel_page}: rendered `conceptual` `{lang}` code block "
                            f"is forbidden under `{top_section}/`"
                        )
                    continue
                if code_text not in verified_snippets:
                    errors.append(
                        f"{rel_page}: rendered `{lang}` code block does not "
                        f"match any verified artifact under anneal/book/examples/"
                    )
                if verified_boring_lines is not None and boring_text:
                    for boring_line in boring_text.splitlines():
                        if boring_line and boring_line not in verified_boring_lines:
                            errors.append(
                                f"{rel_page}: rendered hidden `{lang}` line does not "
                                f"match any verified artifact under anneal/book/examples/: "
                                f"{boring_line!r}"
                            )

        page_ids[resolved] = auditor.ids
        page_hrefs[resolved] = auditor.hrefs
        page_srcs[resolved] = auditor.srcs

    for page, hrefs in sorted(page_hrefs.items()):
        rel_page = page.relative_to(root_resolved)
        for href in hrefs:
            if not href:
                continue
            parsed = urlsplit(href.strip())
            scheme = parsed.scheme.lower()
            if scheme in {"http", "https", "mailto"}:
                continue
            if scheme:
                errors.append(
                    f"{rel_page}: disallowed URL scheme `{scheme}:` in `{href}`"
                )
                continue
            if parsed.netloc:
                errors.append(
                    f"{rel_page}: scheme-relative URL `{href}` is not allowed"
                )
                continue

            path_part = unquote(parsed.path)
            fragment = unquote(parsed.fragment)

            if not path_part:
                target = page
            elif path_part.startswith(site_url_prefix):
                rel = path_part[len(site_url_prefix) :]
                target = (root_resolved / rel).resolve()
            elif path_part.startswith("/"):
                errors.append(
                    f"{rel_page}: absolute link `{href}` does not start with "
                    f"`{site_url_prefix}`"
                )
                continue
            else:
                target = (page.parent / path_part).resolve()

            if not _is_within(target, root_resolved):
                errors.append(f"{rel_page}: link `{href}` escapes built HTML root")
                continue

            if target.is_dir():
                target = target / "index.html"
            if not target.is_file():
                errors.append(f"{rel_page}: broken internal link `{href}`")
                continue

            if fragment and target.suffix == ".html":
                ids = page_ids.get(target.resolve(), set())
                if fragment not in ids:
                    errors.append(
                        f"{rel_page}: broken anchor `#{fragment}` in link `{href}`"
                    )

    for page, srcs in sorted(page_srcs.items()):
        rel_page = page.relative_to(root_resolved)
        for src in srcs:
            stripped_src = src.strip()
            if not stripped_src:
                errors.append(f"{rel_page}: empty image/media source `src`")
                continue
            parsed = urlsplit(stripped_src)
            scheme = parsed.scheme.lower()
            if scheme in {"http", "https"}:
                if not parsed.netloc:
                    errors.append(
                        f"{rel_page}: malformed image/media source `{src}` (missing host)"
                    )
                continue
            if scheme:
                errors.append(
                    f"{rel_page}: disallowed URL scheme `{scheme}:` in media source `{src}`"
                )
                continue
            if parsed.netloc:
                errors.append(
                    f"{rel_page}: scheme-relative media source `{src}` is not allowed"
                )
                continue

            path_part = unquote(parsed.path)
            if not path_part:
                errors.append(f"{rel_page}: broken image/media source `{src}`")
                continue
            if path_part.startswith(site_url_prefix):
                rel = path_part[len(site_url_prefix) :]
                target = (root_resolved / rel).resolve()
            elif path_part.startswith("/"):
                errors.append(
                    f"{rel_page}: absolute image/media source `{src}` does not "
                    f"start with `{site_url_prefix}`"
                )
                continue
            else:
                target = (page.parent / path_part).resolve()

            if not _is_within(target, root_resolved):
                errors.append(
                    f"{rel_page}: image/media source `{src}` escapes built HTML root"
                )
                continue
            if not target.is_file():
                errors.append(f"{rel_page}: broken image/media source `{src}`")
                continue
    return errors


def build_book(
    anneal_root: Path,
    output_dir: Path,
    *,
    mdbook_bin: Path | None = None,
    cache_dir: Path | None = None,
    skip_source_check: bool = False,
) -> list[str]:
    """Stage governing docs, run `mdbook build`, and verify built HTML."""
    if not skip_source_check:
        source_errors = check_sources(anneal_root)
        if source_errors:
            return source_errors

    verified_snippets = _collect_verified_snippet_texts(anneal_root)
    verified_boring_lines = _collect_verified_rust_boring_lines(anneal_root)
    book_src = anneal_root / "book"
    with tempfile.TemporaryDirectory(prefix="anneal-book-stage-") as stage_str:
        stage_root = Path(stage_str)
        resolved_cache = (
            cache_dir if cache_dir is not None else stage_root / "mdbook-cache"
        )
        mdbook = ensure_mdbook(resolved_cache, explicit_bin=mdbook_bin)

        staged_book = stage_root / "book"
        shutil.copytree(book_src, staged_book, symlinks=False)

        for src_name, dest_rel in GOVERNING_DOCS:
            rewritten, link_errors = _rewrite_governing_doc(
                anneal_root / src_name, dest_rel, anneal_root
            )
            if link_errors:
                return link_errors
            dest_path = staged_book / "src" / dest_rel
            dest_path.parent.mkdir(parents=True, exist_ok=True)
            dest_path.write_text(rewritten, encoding="utf-8")

        output_abs = output_dir.resolve()
        if output_abs.exists():
            shutil.rmtree(output_abs)
        output_abs.parent.mkdir(parents=True, exist_ok=True)

        mdbook_env = {
            "PATH": os.environ.get("PATH", ""),
            "HOME": stage_str,
        }
        proc = subprocess.run(
            [str(mdbook), "build", str(staged_book), "--dest-dir", str(output_abs)],
            env=mdbook_env,
            capture_output=True,
            text=True,
            check=False,
        )
        if proc.returncode != 0:
            return [
                f"mdbook build failed (exit {proc.returncode}):\n"
                f"{proc.stdout}\n{proc.stderr}"
            ]

    return check_built_html_links(
        output_abs,
        verified_snippets=verified_snippets,
        verified_boring_lines=verified_boring_lines,
    )


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description="Validate, verify, and build Anneal public documentation."
    )
    parser.add_argument(
        "--anneal-root",
        type=Path,
        default=Path(__file__).resolve().parent.parent,
        help="Path to the anneal/ project root.",
    )
    parser.add_argument(
        "--check-sources",
        action="store_true",
        help="Validate Markdown fences, includes, anchors, examples, and links.",
    )
    parser.add_argument(
        "--verify-examples",
        action="store_true",
        help="Run all examples under book/examples/ against --cargo-anneal.",
    )
    parser.add_argument(
        "--cargo-anneal",
        type=Path,
        help="Path to the cargo-anneal binary for --verify-examples.",
    )
    parser.add_argument(
        "--capture",
        type=Path,
        help="Optional anneal-rustc-capture path for tutor verify cases.",
    )
    parser.add_argument(
        "--sysroot",
        type=Path,
        help="Optional rustc sysroot path for tutor verify cases.",
    )
    parser.add_argument(
        "--lean",
        type=Path,
        help="Optional lean binary path for tutor verify cases.",
    )
    parser.add_argument(
        "--bless",
        action="store_true",
        help="Overwrite example transcripts with live --cargo-anneal output.",
    )
    parser.add_argument(
        "--build",
        action="store_true",
        help="Stage governing docs, build the mdBook site, and check HTML links.",
    )
    parser.add_argument(
        "--output",
        type=Path,
        help="Destination directory for the built HTML site (required with --build).",
    )
    parser.add_argument(
        "--mdbook",
        type=Path,
        help="Path to an existing mdbook binary (skips downloading pinned release).",
    )
    parser.add_argument(
        "--mdbook-cache",
        type=Path,
        help="Cache directory for downloading the pinned mdBook release.",
    )

    args = parser.parse_args(argv)
    if not (args.check_sources or args.verify_examples or args.build):
        parser.error(
            "specify at least one of --check-sources, --verify-examples, or --build"
        )

    if args.bless and not args.verify_examples:
        parser.error("--bless requires --verify-examples")
    if args.verify_examples and args.cargo_anneal is None:
        parser.error("--verify-examples requires --cargo-anneal")
    if args.build and args.output is None:
        parser.error("--build requires --output")

    anneal_root = args.anneal_root.resolve()
    errors: list[str] = []

    def _opt_path(cli_val: Path | None, env_key: str) -> Path | None:
        if cli_val is not None:
            return cli_val.resolve()
        raw = os.environ.get(env_key, "")
        return Path(raw).resolve() if raw else None

    try:
        errors.extend(check_sources(anneal_root))
        if not errors and args.verify_examples:
            capture_path = _opt_path(args.capture, "ANNEAL_CAPTURE")
            sysroot_path = _opt_path(args.sysroot, "ANNEAL_SYSROOT")
            lean_path = _opt_path(args.lean, "ANNEAL_LEAN")
            explicit_tools = (capture_path, sysroot_path, lean_path)
            if any(explicit_tools) and not all(explicit_tools):
                raise DocError(
                    "specify all three of --capture/--sysroot/--lean (or "
                    "ANNEAL_CAPTURE/ANNEAL_SYSROOT/ANNEAL_LEAN), or omit all "
                    "three to use an installed toolchain bundle"
                )
            host_xdg = os.environ.get("XDG_CACHE_HOME", "")
            has_bundle = bool(
                host_xdg and (Path(host_xdg) / "anneal" / "toolchain").is_dir()
            )
            if has_bundle and not any(explicit_tools):
                capture_path = sysroot_path = lean_path = None
            errors.extend(
                verify_examples(
                    anneal_root,
                    args.cargo_anneal.resolve(),
                    bless=args.bless,
                    capture=capture_path,
                    sysroot=sysroot_path,
                    lean=lean_path,
                )
            )
        if not errors and args.build:
            errors.extend(
                build_book(
                    anneal_root,
                    args.output,
                    mdbook_bin=args.mdbook,
                    cache_dir=args.mdbook_cache,
                    skip_source_check=True,
                )
            )
    except DocError as exc:
        errors.append(str(exc))

    if errors:
        for err in errors:
            print(f"error: {err}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
