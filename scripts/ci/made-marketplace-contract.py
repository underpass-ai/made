#!/usr/bin/env python3
"""Validate MADE's co-located Codex/Claude marketplace and release contract."""

from __future__ import annotations

import argparse
import json
import os
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


def verify_release_tag(release_ref: str, require_release_tag: bool) -> None:
    """Branches validate metadata; release invocations also bind it to HEAD."""
    tag_commit = git_output("rev-parse", "--verify", f"refs/tags/{release_ref}^{{commit}}")
    head = git_output("rev-parse", "HEAD")
    if not require_release_tag and (tag_commit is None or tag_commit != head):
        return
    if tag_commit is None:
        fail(f"annotated release tag {release_ref} is not available")
    if git_output("cat-file", "-t", f"refs/tags/{release_ref}") != "tag":
        fail(f"release tag {release_ref} must be annotated")
    if tag_commit != head:
        fail(f"{release_ref} resolves to {tag_commit}, not HEAD {head}")


def verify(require_release_tag: bool = False) -> str:
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
    if codex.get("name") != "made":
        fail("Codex marketplace name must be made (distinct from KMP)")
    if codex_manifest.get("interface", {}).get("websiteURL") != "https://underpassai.com/":
        fail("Codex plugin card must link to https://underpassai.com/")
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
    if claude.get("name") != "made":
        fail("Claude marketplace name must be made (distinct from KMP)")
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

    if os.environ.get("GITHUB_REF_TYPE") == "tag":
        if os.environ.get("GITHUB_REF_NAME") != release_ref:
            fail(f"release ref must match manifest version {release_ref}")
        require_release_tag = True
    verify_release_tag(release_ref, require_release_tag)

    return version


def self_test() -> None:
    import tempfile
    from unittest.mock import patch

    # A future metadata edit must not restore the known catalogue collision or
    # hide the official site. Exercise the same gate used for packaging.
    original_loader = load_json
    for path, field, message in (
        (".agents/plugins/marketplace.json", "name", "distinct from KMP"),
        (".claude-plugin/marketplace.json", "name", "distinct from KMP"),
        ("plugins/made/.codex-plugin/plugin.json", "websiteURL", "must link"),
    ):
        def broken_metadata(relative: str) -> dict:
            value = original_loader(relative)
            if relative == path:
                if field == "name":
                    value["name"] = "underpass"
                else:
                    value["interface"].pop("websiteURL", None)
            return value

        with patch.dict(globals(), load_json=broken_metadata):
            try:
                verify()
            except SystemExit as error:
                assert message in str(error), error
            else:
                raise AssertionError(f"accepted broken marketplace metadata: {path}")

    # Real Git objects distinguish annotated tags, lightweight tags and branches.
    (ROOT / "tmp").mkdir(exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="marketplace-contract-", dir=ROOT / "tmp") as scratch:
        repository = Path(scratch)

        def git(*args: str) -> None:
            subprocess.run(["git", "-C", scratch, *args], check=True, capture_output=True)

        git("init")
        git("config", "user.name", "Contract test")
        git("config", "user.email", "contract@example.invalid")
        git("commit", "--allow-empty", "-m", "release")
        with patch.dict(globals(), ROOT=repository):
            def expect_failure(required: bool, message: str) -> None:
                try:
                    verify_release_tag("v1.2.3", required)
                except SystemExit as error:
                    assert message in str(error), error
                else:
                    raise AssertionError(f"expected refusal: {message}")

            verify_release_tag("v1.2.3", False)
            expect_failure(True, "not available")
            git("tag", "v1.2.3")
            expect_failure(False, "must be annotated")
            git("tag", "-d", "v1.2.3")
            git("tag", "-a", "v1.2.3", "-m", "release")
            verify_release_tag("v1.2.3", False)
            verify_release_tag("v1.2.3", True)
            git("commit", "--allow-empty", "-m", "development")
            verify_release_tag("v1.2.3", False)
            expect_failure(True, "not HEAD")
    print("MADE marketplace self-test passed: 3 discovery and 7 release/branch cases")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--allow-unpublished-tag", action="store_true",
        help="compatibility option: branch checks allow unpublished tags by default",
    )
    parser.add_argument(
        "--require-release-tag", action="store_true",
        help="require the annotated manifest-version tag to resolve to HEAD",
    )
    parser.add_argument("--self-test", action="store_true")
    parser.add_argument("--print-assets", action="store_true")
    args = parser.parse_args()

    if args.self_test:
        self_test()
        return
    if args.allow_unpublished_tag and args.require_release_tag:
        parser.error("--allow-unpublished-tag and --require-release-tag are mutually exclusive")
    version = verify(args.require_release_tag)
    if args.print_assets:
        print("\n".join(expected_assets(version)))
    else:
        print(
            f"MADE marketplace contract passed: made@made {version}, "
            f"co-located plugin tree, {len(expected_assets(version))} release assets"
        )


if __name__ == "__main__":
    main()
