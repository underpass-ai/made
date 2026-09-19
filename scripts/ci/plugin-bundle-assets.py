#!/usr/bin/env python3
"""Validate MADE plugin artwork references in a source or staged bundle."""

from __future__ import annotations

import hashlib
import json
import re
import sys
from pathlib import Path


BRANDING_SHA256 = "9cb1766d30c0f1d5e81bdf1e2831bb002bc750665a1762a0ff70207bdd456115"
BRANDING_NAME = "made-cuatro-voces.png"
SKILLS = {"design-ceremony", "made-setup", "run-ceremony"}
WEBSITE = "https://underpassai.com/"


def fail(message: str) -> None:
    raise SystemExit(f"MADE plugin bundle assets: {message}")


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for chunk in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def resolve_reference(root: Path, base: Path, reference: str, label: str) -> Path:
    if not reference.startswith("./"):
        fail(f"{label} must be a relative ./ path, got {reference!r}")
    resolved = (base / reference[2:]).resolve()
    try:
        resolved.relative_to(root.resolve())
    except ValueError:
        fail(f"{label} escapes the plugin root: {reference!r}")
    if not resolved.is_file():
        fail(f"{label} points at missing file {resolved.relative_to(root).as_posix()}")
    return resolved


def validate(plugin_root: Path) -> dict:
    root = plugin_root.resolve()
    if not root.is_dir():
        fail(f"plugin root is not a directory: {root}")

    codex_path = root / ".codex-plugin/plugin.json"
    try:
        codex = json.loads(codex_path.read_text())
    except (OSError, json.JSONDecodeError) as error:
        fail(f"cannot read {codex_path.relative_to(root).as_posix()}: {error}")
    interface = codex.get("interface", {})
    if interface.get("websiteURL") != WEBSITE:
        fail(f"Codex websiteURL must be {WEBSITE}")

    checked: dict[str, str] = {}
    for field in ("composerIcon", "logo", "logoDark"):
        reference = interface.get(field)
        if not isinstance(reference, str):
            fail(f"Codex manifest omitted interface.{field}")
        path = resolve_reference(root, root, reference, f"interface.{field}")
        digest = sha256(path)
        if digest != BRANDING_SHA256:
            fail(f"interface.{field} artwork digest is {digest}, expected {BRANDING_SHA256}")
        checked[path.relative_to(root).as_posix()] = digest

    skills_root = root / "skills"
    actual_skills = {path.name for path in skills_root.iterdir() if path.is_dir()}
    if actual_skills != SKILLS:
        fail(f"skill set is {sorted(actual_skills)}, expected {sorted(SKILLS)}")
    for skill in sorted(SKILLS):
        skill_root = skills_root / skill
        if not (skill_root / "SKILL.md").is_file():
            fail(f"skills/{skill}/SKILL.md is missing")
        metadata_path = skill_root / "agents/openai.yaml"
        try:
            metadata = metadata_path.read_text()
        except OSError as error:
            fail(f"cannot read skills/{skill}/agents/openai.yaml: {error}")
        for field in ("icon_small", "icon_large"):
            match = re.search(rf'^\s*{field}:\s*["\']([^"\']+)["\']\s*$', metadata, re.MULTILINE)
            if match is None:
                fail(f"skills/{skill}/agents/openai.yaml omitted {field}")
            path = resolve_reference(root, skill_root, match.group(1), f"skills/{skill} {field}")
            digest = sha256(path)
            if path.name != BRANDING_NAME or digest != BRANDING_SHA256:
                fail(f"skills/{skill} {field} does not use the exact C4 branding PNG")
            checked[path.relative_to(root).as_posix()] = digest

    expected_branding_paths = {"assets/made-cuatro-voces.png"} | {
        f"skills/{skill}/assets/made-cuatro-voces.png" for skill in SKILLS
    }
    if not expected_branding_paths.issubset(checked):
        fail(f"unverified branding paths: {sorted(expected_branding_paths - set(checked))}")
    return {
        "plugin_root": str(root),
        "website": WEBSITE,
        "skills": sorted(SKILLS),
        "branding_sha256": BRANDING_SHA256,
        "verified_artwork": dict(sorted(checked.items())),
    }


def main() -> None:
    if len(sys.argv) != 2:
        raise SystemExit("usage: plugin-bundle-assets.py PLUGIN_ROOT")
    print(json.dumps(validate(Path(sys.argv[1])), indent=2))


if __name__ == "__main__":
    main()
