#!/bin/sh
set -eu

output=
url=
while [ "$#" -gt 0 ]; do
  case "$1" in
    -o)
      shift
      output=${1:-}
      ;;
    https://*) url=$1 ;;
  esac
  shift
done

[ -n "$output" ] && [ -n "$url" ] || {
  echo "fake release curl: expected an HTTPS URL and -o path" >&2
  exit 2
}
: "${MADE_FAKE_CURL_SOURCE:?MADE_FAKE_CURL_SOURCE is required}"
: "${MADE_FAKE_CURL_REQUESTS:?MADE_FAKE_CURL_REQUESTS is required}"
printf '%s\n' "$url" >>"$MADE_FAKE_CURL_REQUESTS"

case "$url" in
  *.sha256)
    if [ "${MADE_FAKE_CURL_BAD_CHECKSUM:-0}" = 1 ]; then
      printf '%064d  made-mcp\n' 0 >"$output"
    elif command -v sha256sum >/dev/null 2>&1; then
      digest=$(sha256sum "$MADE_FAKE_CURL_SOURCE" | awk '{ print $1 }')
      printf '%s  made-mcp\n' "$digest" >"$output"
    else
      digest=$(shasum -a 256 "$MADE_FAKE_CURL_SOURCE" | awk '{ print $1 }')
      printf '%s  made-mcp\n' "$digest" >"$output"
    fi
    ;;
  *) cp "$MADE_FAKE_CURL_SOURCE" "$output" ;;
esac
