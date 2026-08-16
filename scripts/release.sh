#!/usr/bin/env bash
set -euo pipefail

# Release helper for MADE.
#
# Two verbs:
#   version <X.Y.Z>   — rewrite every versioned artefact in the repo
#                       so Cargo.toml and Chart.yaml stay in lockstep.
#                       Idempotent; safe to re-run.
#
#   release <X.Y.Z>   — verify the tree is clean + versions already
#                       point at X.Y.Z, then create a signed `vX.Y.Z`
#                       tag at HEAD and push it. The publish-
#                       distribution workflow takes it from there
#                       (container image + Helm chart to ghcr).
#
# Typical flow:
#   just version 0.2.0
#   just check                  # fast gates green
#   just integration            # container-backed gates green
#   make e2e-compose            # end-to-end sanity
#   make e2e-kubernetes         # kubernetes job sanity
#   git commit -am "chore: v0.2.0"
#   gh pr create --fill
#   # merge via CI
#   git checkout main && git pull
#   just release 0.2.0

usage() {
    cat <<'USAGE' >&2
release.sh version <X.Y.Z>
release.sh release <X.Y.Z>
USAGE
    exit 2
}

semver_check() {
    local version="$1"
    if ! echo "${version}" | grep -Eq '^[0-9]+\.[0-9]+\.[0-9]+(-[A-Za-z0-9.-]+)?$'; then
        echo "error: version '${version}' is not valid semver" >&2
        exit 1
    fi
}

cmd_version() {
    local version="$1"
    semver_check "${version}"

    local root
    root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
    cd "${root}"

    # Workspace Cargo.toml — the [workspace.package] version line.
    # Match only the first occurrence so we don't rewrite deps.
    python3 - "${version}" <<'PY'
import pathlib, re, sys

version = sys.argv[1]

# Cargo.toml: workspace.package.version (first occurrence only).
cargo = pathlib.Path("Cargo.toml")
text = cargo.read_text()
new_text, count = re.subn(
    r'(^version = )"[^"]+"',
    rf'\1"{version}"',
    text,
    count=1,
    flags=re.MULTILINE,
)
if count == 0:
    sys.exit("Cargo.toml: no workspace version line matched")

# Internal path dependencies carry a literal version next to their path —
# cargo has no way to inherit it — and a published crate whose sibling
# requirement still points at the previous release cannot resolve on
# crates.io. They move with the workspace or the release is broken.
new_text, pinned = re.subn(
    r'(^made-[a-z-]+ = \{ path = "crates/[^"]+", version = )"[^"]+"',
    rf'\1"{version}"',
    new_text,
    flags=re.MULTILINE,
)
if pinned == 0:
    sys.exit("Cargo.toml: no internal dependency pins matched")
cargo.write_text(new_text)

# Chart.yaml: both `version:` (chart) and `appVersion:` (app) track
# the binary's own version. Kept in lockstep intentionally.
chart = pathlib.Path("charts/made/Chart.yaml")
text = chart.read_text()
text, c1 = re.subn(r'^version:.*$', f'version: {version}', text, count=1, flags=re.MULTILINE)
text, c2 = re.subn(r'^appVersion:.*$', f'appVersion: "{version}"', text, count=1, flags=re.MULTILINE)
if c1 == 0 or c2 == 0:
    sys.exit("Chart.yaml: version / appVersion line missing")
chart.write_text(text)

# The plugin host manifests. The packaging script stamps these when it
# builds a release bundle, but a host can also install straight from this
# repository, so whatever is committed here is the version those users
# see. Left behind, it pins them at the first release forever: a plugin
# update reports "already at the latest version" no matter what ships,
# because the number never moves.
manifests = [
    pathlib.Path("plugins/made/.claude-plugin/plugin.json"),
    pathlib.Path("plugins/made/.codex-plugin/plugin.json"),
]
for manifest in manifests:
    text = manifest.read_text()
    text, c = re.subn(
        r'(^  "version": )"[^"]+"',
        rf'\1"{version}"',
        text,
        count=1,
        flags=re.MULTILINE,
    )
    if c == 0:
        sys.exit(f"{manifest}: no version line matched")
    manifest.write_text(text)

print(
    f"bumped to {version}: Cargo.toml, charts/made/Chart.yaml, "
    f"{len(manifests)} plugin manifests"
)
PY

    # Surface what changed — caller reviews before committing.
    # Cargo.lock records the workspace members' own versions. Without this
    # the very next `cargo test --locked` — which is what CI runs — fails
    # with "cannot update the lock file because --locked was passed".
    cargo metadata --format-version 1 >/dev/null

    git --no-pager diff --stat -- Cargo.toml Cargo.lock charts/made/Chart.yaml \
        plugins/made/.claude-plugin/plugin.json plugins/made/.codex-plugin/plugin.json
}

cmd_release() {
    local version="$1"
    semver_check "${version}"

    local root
    root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
    cd "${root}"

    # Tree must be clean; releasing on top of uncommitted changes
    # would ship an image whose commit sha doesn't reflect reality.
    if [ -n "$(git status --porcelain)" ]; then
        echo "error: working tree is dirty — commit or stash first" >&2
        git status --short >&2
        exit 1
    fi

    # Versions must already match — the `version` verb is where you
    # bump; `release` only tags.
    local cargo_version chart_version chart_app_version
    cargo_version="$(grep -m1 '^version = ' Cargo.toml | sed -E 's/version = "([^"]+)"/\1/')"
    chart_version="$(grep -m1 '^version:' charts/made/Chart.yaml | awk '{print $2}')"
    chart_app_version="$(grep -m1 '^appVersion:' charts/made/Chart.yaml | awk '{print $2}' | tr -d '"')"

    for field in cargo_version chart_version chart_app_version; do
        if [ "${!field}" != "${version}" ]; then
            echo "error: ${field}='${!field}' does not match target '${version}'" >&2
            echo "  hint: run 'just version ${version}' and commit before releasing" >&2
            exit 1
        fi
    done

    local tag="v${version}"
    if git rev-parse -q --verify "refs/tags/${tag}" >/dev/null; then
        echo "error: tag ${tag} already exists" >&2
        exit 1
    fi

    # Current branch must be main — release tags only come off the
    # reviewed history.
    local branch
    branch="$(git rev-parse --abbrev-ref HEAD)"
    if [ "${branch}" != "main" ]; then
        echo "error: not on main (currently '${branch}')" >&2
        exit 1
    fi

    git tag -a "${tag}" -m "Release ${tag}"
    git push origin "${tag}"
    echo "tagged ${tag} and pushed. publish-distribution will build image + chart."
}

if [ $# -lt 1 ]; then
    usage
fi

verb="$1"
shift

case "${verb}" in
    version)
        [ $# -eq 1 ] || usage
        cmd_version "$1"
        ;;
    release)
        [ $# -eq 1 ] || usage
        cmd_release "$1"
        ;;
    *)
        usage
        ;;
esac
