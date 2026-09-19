#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 1 ]]; then
  echo "usage: prepare-auth.sh <output-directory>" >&2
  exit 2
fi
command -v openssl >/dev/null 2>&1 || {
  echo "openssl is required to prepare the compose mTLS fixture" >&2
  exit 1
}

output="$1"
mkdir -p "$output"
umask 077

openssl req -x509 -newkey rsa:2048 -nodes -days 1 \
  -subj /CN=made-e2e-ca \
  -keyout "$output/ca.key" -out "$output/ca.pem" >/dev/null 2>&1

openssl req -newkey rsa:2048 -nodes -subj /CN=made \
  -keyout "$output/server.key" -out "$output/server.csr" >/dev/null 2>&1
cat >"$output/server.ext" <<'EOF'
subjectAltName=DNS:made
extendedKeyUsage=serverAuth
EOF
openssl x509 -req -days 1 -sha256 -CA "$output/ca.pem" -CAkey "$output/ca.key" \
  -CAcreateserial -in "$output/server.csr" -out "$output/server.pem" \
  -extfile "$output/server.ext" >/dev/null 2>&1

openssl req -newkey rsa:2048 -nodes -subj /CN=made-e2e-owner \
  -keyout "$output/client.key" -out "$output/client.csr" >/dev/null 2>&1
cat >"$output/client.ext" <<'EOF'
extendedKeyUsage=clientAuth
EOF
openssl x509 -req -days 1 -sha256 -CA "$output/ca.pem" -CAkey "$output/ca.key" \
  -CAcreateserial -in "$output/client.csr" -out "$output/client.pem" \
  -extfile "$output/client.ext" >/dev/null 2>&1

fingerprint="$({ openssl x509 -in "$output/client.pem" -outform DER; } | sha256sum | cut -d' ' -f1)"
cat >"$output/principals.json" <<EOF
[
  {
    "certificate_sha256": "$fingerprint",
    "principal_id": "made-e2e-owner",
    "principal_kind": "trusted_host"
  }
]
EOF

# Both containers run as non-root. These are one-day test credentials in a
# per-run scratch directory, never production material.
chmod 0444 "$output/ca.pem" "$output/server.pem" "$output/server.key" \
  "$output/client.pem" "$output/client.key" "$output/principals.json"
rm -f "$output/ca.key" "$output/ca.srl" "$output/server.csr" \
  "$output/server.ext" "$output/client.csr" "$output/client.ext"
# mktemp creates its directory as 0700. The credentials themselves remain
# read-only, but the non-root service and runner must be able to traverse the
# bind mount to read them.
chmod 0755 "$output"
