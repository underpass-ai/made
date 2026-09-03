#!/usr/bin/env python3
"""Validate MADE's co-located Codex/Claude marketplace and release contract."""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
SEMVER = re.compile(r"^[0-9]+\.[0-9]+\.[0-9]+(?:-[0-9A-Za-z.-]+)?$")


def fail(message: str) -> None:
    raise SystemExit(f"MADE marketplace contract: {message}")


def load_json(relative: str) -> dict:
    path = ROOT / relative
    try:
        value = json.loads(path.read_text())
    except (OSError, json.JSONDecodeError) as error:
        fail(f"cannot read {relative}: {error}")
    if not isinstance(value, dict):
        fail(f"{relative} must contain a JSON object")
    return value


def workspace_version() -> str:
    match = re.search(
        r'^version = "([^"]+)"',
        (ROOT / "Cargo.toml").read_text(),
        flags=re.MULTILINE,
    )
    if not match or not SEMVER.fullmatch(match.group(1)):
        fail("Cargo.toml has no strict workspace semver")
    return match.group(1)


def expected_assets(version: str) -> list[str]:
    platforms = [
        ("linux-x86_64", "x86_64-unknown-linux-gnu", ""),
        ("linux-arm64", "aarch64-unknown-linux-gnu", ""),
        ("macos-arm64", "aarch64-apple-darwin", ""),
        ("windows-x86_64", "x86_64-pc-windows-msvc", ".exe"),
    ]
    assets: list[str] = []
    for plugin_label, target, suffix in platforms:
        archive = f"made-plugin-{version}-{plugin_label}.tar.gz"
        binary = f"made-mcp-v{version}-{target}{suffix}"
        assets.extend([archive, f"{archive}.sha256", binary, f"{binary}.sha256"])
    return sorted(assets)


def git_output(*args: str) -> str | None:
    result = subprocess.run(
        ["git", *args],
        cwd=ROOT,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.DEVNULL,
        check=False,
    )
    return result.stdout.strip() if result.returncode == 0 else None


def verify(allow_unpublished_tag: bool) -> str:
    version = workspace_version()
    release_ref = f"v{version}"

    codex_manifest = load_json("plugins/made/.codex-plugin/plugin.json")
    claude_manifest = load_json("plugins/made/.claude-plugin/plugin.json")
    for label, manifest in (("Codex", codex_manifest), ("Claude", claude_manifest)):
        if manifest.get("name") != "made":
            fail(f"{label} manifest name must be made")
        if manifest.get("version") != version:
            fail(f"{label} manifest version must be {version}")
    if claude_manifest.get("commands") != "./claude/commands/":
        fail("Claude manifest must expose ./claude/commands/")

    prompts = codex_manifest.get("interface", {}).get("defaultPrompt", [])
    if not isinstance(prompts, list) or len(prompts) > 3:
        fail("Codex defaultPrompt must contain at most three prompts")

    codex = load_json(".agents/plugins/marketplace.json")
    if codex.get("name") != "underpass":
        fail("Codex marketplace name must be underpass")
    codex_plugins = codex.get("plugins")
    if not isinstance(codex_plugins, list) or len(codex_plugins) != 1:
        fail("Codex marketplace must contain exactly one plugin")
    codex_entry = codex_plugins[0]
    if codex_entry.get("name") != "made":
        fail("Codex marketplace plugin must be made")
    if codex_entry.get("source") != {"source": "local", "path": "./plugins/made"}:
        fail("Codex marketplace must resolve ./plugins/made")
    if codex_entry.get("policy") != {
        "installation": "AVAILABLE",
        "authentication": "ON_INSTALL",
    }:
        fail("Codex marketplace policy must be AVAILABLE/ON_INSTALL")
    if codex_entry.get("category") != "Developer Tools":
        fail("Codex marketplace category must be Developer Tools")

    claude = load_json(".claude-plugin/marketplace.json")
    if claude.get("name") != "underpass":
        fail("Claude marketplace name must be underpass")
    claude_plugins = claude.get("plugins")
    if not isinstance(claude_plugins, list) or len(claude_plugins) != 1:
        fail("Claude marketplace must contain exactly one plugin")
    claude_entry = claude_plugins[0]
    source = claude_entry.get("source")
    if claude_entry.get("name") != "made" or source != {
        "source": "git-subdir",
        "url": "https://github.com/underpass-ai/made.git",
        "path": "plugins/made",
        "ref": release_ref,
    }:
        fail(f"Claude marketplace must pin plugins/made to immutable {release_ref}")

    tracked_bin = git_output("ls-files", "--", "plugins/made/bin")
    if tracked_bin:
        fail("plugins/made/bin must remain untracked; setup owns the binary")

    required = [
        "plugins/made/scripts/made-install-binary.sh",
        "plugins/made/scripts/made-install-binary.ps1",
        "plugins/made/skills/made-setup/SKILL.md",
        "plugins/made/claude/commands/setup.md",
    ]
    for relative in required:
        if not (ROOT / relative).is_file():
            fail(f"missing {relative}")
    setup_index_entry = git_output("ls-files", "-s", "--", required[0])
    if not setup_index_entry or not setup_index_entry.startswith("100755 "):
        fail("POSIX setup adapter must be tracked as executable")

    package_text = (ROOT / "scripts/plugin/package-made-plugin.sh").read_text()
    posix_setup = (ROOT / required[0]).read_text()
    windows_setup = (ROOT / required[1]).read_text()
    for target in (
        "x86_64-unknown-linux-gnu",
        "aarch64-unknown-linux-gnu",
        "aarch64-apple-darwin",
        "x86_64-pc-windows-msvc",
    ):
        if target not in package_text:
            fail(f"packager does not map standalone target {target}")
        if target not in posix_setup and target not in windows_setup:
            fail(f"setup does not map standalone target {target}")

    advance_text = (ROOT / "scripts/release/advance-marketplace.sh").read_text()
    release_text = (ROOT / "scripts/release.sh").read_text()
    compare_at = advance_text.find('cmp -s "${SCRATCH}/expected.txt"')
    push_at = advance_text.find('git push origin "${HEAD_COMMIT}:refs/heads/marketplace"')
    if compare_at < 0 or push_at < 0 or compare_at >= push_at:
        fail("marketplace advance must follow exact public-asset comparison")
    if "--force" in advance_text:
        fail("marketplace branch advance must never force-push")
    if 'bash scripts/release/advance-marketplace.sh "${version}"' not in release_text:
        fail("release command does not wait for assets and advance marketplace")

    tag_commit = git_output("rev-parse", "--verify", f"refs/tags/{release_ref}^{{commit}}")
    if tag_commit is None:
        if not allow_unpublished_tag:
            fail(f"annotated release tag {release_ref} is not available")
    else:
        if git_output("cat-file", "-t", f"refs/tags/{release_ref}") != "tag":
            fail(f"release tag {release_ref} must be annotated")
        head = git_output("rev-parse", "HEAD")
        if tag_commit != head:
            fail(f"{release_ref} resolves to {tag_commit}, not HEAD {head}")
        difference = subprocess.run(
            ["git", "diff", "--quiet", tag_commit, "HEAD", "--", "plugins/made"],
            cwd=ROOT,
            check=False,
        )
        if difference.returncode != 0:
            fail("Claude and Codex marketplace mappings do not resolve the same plugin tree")

    return version


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--allow-unpublished-tag", action="store_true")
    parser.add_argument("--print-assets", action="store_true")
    args = parser.parse_args()

    version = verify(args.allow_unpublished_tag)
    if args.print_assets:
        print("\n".join(expected_assets(version)))
    else:
        print(
            f"MADE marketplace contract passed: made@underpass {version}, "
            f"co-located plugin tree, {len(expected_assets(version))} release assets"
        )


if __name__ == "__main__":
    main()
