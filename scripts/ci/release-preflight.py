#!/usr/bin/env python3
"""Fail closed before any MADE release publication mutates a registry."""

from __future__ import annotations

import argparse
import json
import os
import pathlib
import re
import subprocess
import sys
import tempfile
import tomllib


ROOT = pathlib.Path(__file__).resolve().parents[2]
SEMVER = re.compile(
    r"^(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)"
    r"(?:-(?P<prerelease>[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*))?$"
)


class PreflightError(RuntimeError):
    """A release identity is inconsistent or cannot be proved."""


def is_semver(value: object) -> bool:
    if not isinstance(value, str):
        return False
    match = SEMVER.fullmatch(value)
    if match is None:
        return False
    prerelease = match.group("prerelease")
    if prerelease is None:
        return True
    return all(
        not (identifier.isdigit() and len(identifier) > 1 and identifier.startswith("0"))
        for identifier in prerelease.split(".")
    )


def load_toml(root: pathlib.Path, relative: str) -> dict:
    try:
        return tomllib.loads((root / relative).read_text())
    except (OSError, tomllib.TOMLDecodeError) as error:
        raise PreflightError(f"cannot read {relative}: {error}") from error


def load_json(root: pathlib.Path, relative: str) -> dict:
    try:
        value = json.loads((root / relative).read_text())
    except (OSError, json.JSONDecodeError) as error:
        raise PreflightError(f"cannot read {relative}: {error}") from error
    if not isinstance(value, dict):
        raise PreflightError(f"{relative} must contain a JSON object")
    return value


def git(root: pathlib.Path, *args: str) -> str | None:
    result = subprocess.run(
        ["git", *args],
        cwd=root,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.DEVNULL,
        check=False,
    )
    return result.stdout.strip() if result.returncode == 0 else None


def release_identity(root: pathlib.Path) -> tuple[str, list[str]]:
    cargo = load_toml(root, "Cargo.toml")
    workspace = cargo.get("workspace", {})
    package = workspace.get("package", {})
    version = package.get("version")
    if not is_semver(version):
        raise PreflightError("Cargo.toml workspace version is not strict SemVer")

    members = workspace.get("members")
    if not isinstance(members, list) or not members:
        raise PreflightError("Cargo.toml workspace has no members")
    member_names: list[str] = []
    for member in members:
        manifest = load_toml(root, f"{member}/Cargo.toml")
        member_package = manifest.get("package", {})
        name = member_package.get("name")
        if not isinstance(name, str):
            raise PreflightError(f"{member}/Cargo.toml has no package name")
        if member_package.get("version") != {"workspace": True}:
            raise PreflightError(
                f"{member}/Cargo.toml must inherit the workspace version"
            )
        member_names.append(name)

    dependencies = workspace.get("dependencies", {})
    for name, dependency in dependencies.items():
        if not name.startswith("made-") or not isinstance(dependency, dict):
            continue
        if "path" in dependency and dependency.get("version") != version:
            raise PreflightError(
                f"workspace dependency {name} pins {dependency.get('version')!r}, "
                f"expected {version}"
            )

    lock = load_toml(root, "Cargo.lock")
    local_versions = {
        item.get("name"): item.get("version")
        for item in lock.get("package", [])
        if isinstance(item, dict) and "source" not in item
    }
    for name in member_names:
        if local_versions.get(name) != version:
            raise PreflightError(
                f"Cargo.lock workspace package {name} is "
                f"{local_versions.get(name)!r}, expected {version}"
            )

    # Keep the release preflight dependency-free; these two scalar YAML fields
    # have no aliases or templating and can be validated exactly.
    chart_text = (root / "charts/made/Chart.yaml").read_text()
    chart_version = re.search(r"^version:\s*([^\s]+)\s*$", chart_text, re.MULTILINE)
    app_version = re.search(
        r'^appVersion:\s*["\']?([^"\'\s]+)["\']?\s*$',
        chart_text,
        re.MULTILINE,
    )
    if chart_version is None or chart_version.group(1) != version:
        raise PreflightError("Helm chart version does not match the workspace")
    if app_version is None or app_version.group(1) != version:
        raise PreflightError("Helm appVersion does not match the workspace")

    for relative in (
        "plugins/made/.codex-plugin/plugin.json",
        "plugins/made/.claude-plugin/plugin.json",
    ):
        if load_json(root, relative).get("version") != version:
            raise PreflightError(f"{relative} version does not match the workspace")

    catalog = load_json(root, ".claude-plugin/marketplace.json")
    entries = [
        item
        for item in catalog.get("plugins", [])
        if isinstance(item, dict) and item.get("name") == "made"
    ]
    if len(entries) != 1:
        raise PreflightError("Claude marketplace must contain one made entry")
    source = entries[0].get("source")
    if not isinstance(source, dict) or source.get("ref") != f"v{version}":
        raise PreflightError(f"Claude marketplace must pin v{version}")

    return version, sorted(member_names)


def verify_tag(root: pathlib.Path, release_ref: str) -> None:
    tag_type = git(root, "cat-file", "-t", f"refs/tags/{release_ref}")
    if tag_type != "tag":
        raise PreflightError(f"{release_ref} must be an available annotated tag")
    tag_commit = git(root, "rev-parse", f"refs/tags/{release_ref}^{{commit}}")
    head = git(root, "rev-parse", "HEAD")
    if tag_commit is None or head is None or tag_commit != head:
        raise PreflightError(f"{release_ref} must resolve to HEAD")


def expected_plugin_assets(version: str) -> list[str]:
    platforms = (
        ("linux-x86_64", "x86_64-unknown-linux-gnu", ""),
        ("linux-arm64", "aarch64-unknown-linux-gnu", ""),
        ("macos-arm64", "aarch64-apple-darwin", ""),
        ("windows-x86_64", "x86_64-pc-windows-msvc", ".exe"),
    )
    assets: list[str] = []
    for label, target, suffix in platforms:
        archive = f"made-plugin-{version}-{label}.tar.gz"
        binary = f"made-mcp-v{version}-{target}{suffix}"
        assets.extend((archive, f"{archive}.sha256", binary, f"{binary}.sha256"))
    return sorted(assets)


def self_test() -> None:
    source_root = ROOT
    version, members = release_identity(source_root)
    if not is_semver("0.7.0-rc.1"):
        raise AssertionError("refused the planned release-candidate identity")
    for invalid in ("0.7.0-01", "0.7.0+rebuilt"):
        if is_semver(invalid):
            raise AssertionError(f"accepted unsupported release identity {invalid}")
    if not members or len(expected_plugin_assets(version)) != 16:
        raise AssertionError("current release identity did not produce its full inventory")

    (source_root / "tmp").mkdir(exist_ok=True)
    with tempfile.TemporaryDirectory(
        prefix="release-preflight-", dir=source_root / "tmp"
    ) as directory:
        root = pathlib.Path(directory)
        for relative in (
            "Cargo.toml",
            "Cargo.lock",
            "charts/made/Chart.yaml",
            "plugins/made/.codex-plugin/plugin.json",
            "plugins/made/.claude-plugin/plugin.json",
            ".claude-plugin/marketplace.json",
        ):
            destination = root / relative
            destination.parent.mkdir(parents=True, exist_ok=True)
            destination.write_bytes((source_root / relative).read_bytes())
        cargo = load_toml(source_root, "Cargo.toml")
        for member in cargo["workspace"]["members"]:
            destination = root / member / "Cargo.toml"
            destination.parent.mkdir(parents=True, exist_ok=True)
            destination.write_bytes((source_root / member / "Cargo.toml").read_bytes())

        manifest = root / "plugins/made/.codex-plugin/plugin.json"
        manifest.write_text(manifest.read_text().replace(version, "9.9.9", 1))
        try:
            release_identity(root)
        except PreflightError as error:
            assert "version does not match" in str(error), error
        else:
            raise AssertionError("accepted a drifted plugin manifest")

        manifest.write_bytes(
            (source_root / "plugins/made/.codex-plugin/plugin.json").read_bytes()
        )
        chart = root / "charts/made/Chart.yaml"
        chart.write_text(chart.read_text().replace(f"version: {version}", "version: 9.9.9", 1))
        try:
            release_identity(root)
        except PreflightError as error:
            assert "chart version" in str(error), error
        else:
            raise AssertionError("accepted a drifted Helm chart")

    with tempfile.TemporaryDirectory(
        prefix="release-tag-", dir=source_root / "tmp"
    ) as directory:
        repository = pathlib.Path(directory)

        def run_git(*arguments: str) -> None:
            subprocess.run(
                ["git", *arguments], cwd=repository, check=True, capture_output=True
            )

        run_git("init")
        run_git("config", "user.name", "Release preflight")
        run_git("config", "user.email", "release-preflight@example.invalid")
        run_git("commit", "--allow-empty", "-m", "candidate")
        run_git("tag", "v0.7.0-rc.1")
        try:
            verify_tag(repository, "v0.7.0-rc.1")
        except PreflightError as error:
            assert "annotated" in str(error), error
        else:
            raise AssertionError("accepted a lightweight release tag")
        run_git("tag", "-d", "v0.7.0-rc.1")
        run_git("tag", "-a", "v0.7.0-rc.1", "-m", "candidate")
        verify_tag(repository, "v0.7.0-rc.1")
        run_git("commit", "--allow-empty", "-m", "later")
        try:
            verify_tag(repository, "v0.7.0-rc.1")
        except PreflightError as error:
            assert "resolve to HEAD" in str(error), error
        else:
            raise AssertionError("accepted a release tag from another commit")

    print(
        f"release preflight self-test passed: {len(members)} workspace members, "
        "lock/chart/plugin drift and mutable tags refused"
    )


def main() -> None:
    parser = argparse.ArgumentParser()
    tag = parser.add_mutually_exclusive_group(required=False)
    tag.add_argument("--require-release-tag", action="store_true")
    tag.add_argument("--allow-unpublished-tag", action="store_true")
    parser.add_argument("--github-output", type=pathlib.Path)
    parser.add_argument("--plugin-assets", type=pathlib.Path)
    parser.add_argument("--expect-version")
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()

    if args.self_test:
        self_test()
        return

    try:
        version, members = release_identity(ROOT)
        if args.expect_version is not None and args.expect_version != version:
            raise PreflightError(
                f"requested version {args.expect_version} does not match {version}"
            )
        release_ref = f"v{version}"
        if args.require_release_tag:
            verify_tag(ROOT, release_ref)
        if os.environ.get("GITHUB_REF_TYPE") == "tag":
            if os.environ.get("GITHUB_REF_NAME") != release_ref:
                raise PreflightError(
                    f"workflow tag {os.environ.get('GITHUB_REF_NAME')!r} "
                    f"does not match {release_ref}"
                )
            verify_tag(ROOT, release_ref)
    except (OSError, PreflightError) as error:
        raise SystemExit(f"release preflight: {error}") from error

    prerelease = "-" in version
    if args.github_output is not None:
        with args.github_output.open("a") as output:
            output.write(f"version={version}\n")
            output.write(f"release_ref={release_ref}\n")
            output.write(f"prerelease={str(prerelease).lower()}\n")
    if args.plugin_assets is not None:
        args.plugin_assets.parent.mkdir(parents=True, exist_ok=True)
        args.plugin_assets.write_text("\n".join(expected_plugin_assets(version)) + "\n")

    print(
        f"release preflight passed: {release_ref}, prerelease={str(prerelease).lower()}, "
        f"{len(members)} workspace members"
    )


if __name__ == "__main__":
    main()
