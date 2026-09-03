#!/bin/sh
set -eu

plugin_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
manifest="$plugin_root/.codex-plugin/plugin.json"
[ -f "$manifest" ] || manifest="$plugin_root/.claude-plugin/plugin.json"

version=$(sed -n 's/.*"version"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' \
  "$manifest" | head -n 1)
version=${version%%+*}
[ -n "$version" ] || {
  echo "MADE setup: plugin manifest has no version" >&2
  exit 127
}

case "$(uname -s)-$(uname -m)" in
  Linux-x86_64)
    target=x86_64-unknown-linux-gnu
    binary_name=made-mcp
    ;;
  Linux-aarch64)
    target=aarch64-unknown-linux-gnu
    binary_name=made-mcp
    ;;
  Darwin-arm64)
    target=aarch64-apple-darwin
    binary_name=made-mcp
    ;;
  MINGW*-x86_64|MSYS*-x86_64|CYGWIN*-x86_64)
    target=x86_64-pc-windows-msvc
    binary_name=made-mcp.exe
    ;;
  *)
    echo "MADE setup: no prebuilt made-mcp for this platform" >&2
    echo "MADE setup: supported targets are Linux x86_64/arm64, macOS arm64, and Windows x86_64" >&2
    exit 127
    ;;
esac

install_dir=${MADE_INSTALL_DIR:-"$plugin_root/bin"}
binary="$install_dir/$binary_name"

installed_version() {
  "$1" --version 2>/dev/null | sed -n '1s/^made-mcp \([^ ]*\).*/\1/p'
}

if [ "${MADE_SETUP_FORCE:-0}" != 1 ] && [ -x "$binary" ]; then
  actual=$(installed_version "$binary")
  if [ "$actual" = "$version" ]; then
    echo "MADE setup: made-mcp $version is ready at $binary"
    exit 0
  fi
fi

command -v curl >/dev/null 2>&1 || {
  echo "MADE setup: curl is required to download the release binary" >&2
  exit 127
}

scratch=$(mktemp -d)
trap 'rm -rf "$scratch"' EXIT HUP INT TERM
asset="made-mcp-v${version}-${target}"
[ "$binary_name" = made-mcp.exe ] && asset="${asset}.exe"
base="https://github.com/underpass-ai/made/releases/download/v${version}/${asset}"

curl --proto '=https' --tlsv1.2 -fsSL "$base" -o "$scratch/$binary_name"
curl --proto '=https' --tlsv1.2 -fsSL "$base.sha256" -o "$scratch/$binary_name.sha256"

published=$(awk 'NR == 1 { print tolower($1) }' "$scratch/$binary_name.sha256")
if command -v sha256sum >/dev/null 2>&1; then
  actual=$(sha256sum "$scratch/$binary_name" | awk '{ print tolower($1) }')
elif command -v shasum >/dev/null 2>&1; then
  actual=$(shasum -a 256 "$scratch/$binary_name" | awk '{ print tolower($1) }')
else
  echo "MADE setup: sha256sum or shasum is required to verify the release binary" >&2
  exit 127
fi

[ -n "$published" ] && [ "$published" = "$actual" ] || {
  echo "MADE setup: checksum mismatch for $asset" >&2
  exit 1
}

mkdir -p "$install_dir"
staged="$install_dir/.${binary_name}.tmp.$$"
install -m 755 "$scratch/$binary_name" "$staged"
mv -f "$staged" "$binary"

actual=$(installed_version "$binary")
[ "$actual" = "$version" ] || {
  echo "MADE setup: installed binary reports '$actual', expected '$version'" >&2
  exit 1
}

echo "MADE setup: installed and verified made-mcp $version at $binary"
