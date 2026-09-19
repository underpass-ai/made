# Operator console

The `made-console` binary reads and acts through MADE's public gRPC contract.
It never opens SQLite, Postgres, artifact paths or internal service endpoints.
Its reusable transport lives in `made-client`.

## Install and connect

Install a published version with Cargo and verify the binary before using it:

```bash
cargo install made-console --locked
made-console --version
made-console --help
```

For an unreleased source candidate, use an isolated install root inside the
checkout:

```bash
cargo install --path crates/made-console --locked --root ./tmp/made-console
./tmp/made-console/bin/made-console --version
```

On Windows the installed program is `made-console.exe`. The console does not
start a MADE service; configure the service's public gRPC endpoint explicitly:

```bash
export MADE_ENDPOINT=http://127.0.0.1:50055
made-console get CEREMONY_ID
```

Production gRPC uses mutual TLS. The server maps the client certificate's
SHA-256 fingerprint through `MADE_AUTH_MTLS_PRINCIPALS_PATH`; clients never
assert a principal in metadata:

```bash
export MADE_ENDPOINT=https://made.example:50055
export MADE_TLS_CA_CERTIFICATE=./ca.pem
export MADE_TLS_CLIENT_CERTIFICATE=./operator-cert.pem
export MADE_TLS_CLIENT_KEY=./operator-key.pem
made-console get CEREMONY_ID
```

The console creates one random namespace per invocation, then derives each
`x-made-request-id` from that namespace, the exact RPC method and canonical
protobuf payload. Two actions or chunk offsets receive different ids; an
identical reconstruction receives the same one. Supply `--request-namespace`
or `MADE_REQUEST_NAMESPACE` when retrying an ambiguous invocation. Request ids
are correlation and idempotency input, never authentication.

`scripts/ci/package-made-console.sh` builds a native release candidate,
checks its package manifests, version, top-level help and artifact subcommands,
then writes the binary and SHA-256 file under `dist/console/`. It never uploads
or creates a tag.

The useful recovery path is `watch --cursor-file PATH --follow`. The cursor
file contains the ceremony id and last confirmed G6 sequence. On restart the
console checks the ceremony scope, replays from that sequence and deduplicates
durable event ids within each bounded response. The journal is still the
authority; the file is only a delivery checkpoint.

Tree reads start from one ceremony id and follow public child ids up to
`--max-nodes`. Artifact listing preserves the opaque server cursor. Artifact
and report exports write a temporary file in the destination directory and
install it only after the response is complete; artifact export also verifies
chunk offsets, chunk digests, final size and final digest.

The console exposes lifecycle actions supported by the public API: pause,
resume, cancel, deadline enforcement and guard approval. Admission uses the
principal authenticated by mTLS and the server's policy; actor-shaped payload
fields are provenance only and cannot override that principal.

`budget report CEREMONY_ID` reads the durable account shared by the ceremony
tree and identifies bounded dimensions that are exhausted or overrun.
`budget pending` pages unresolved reservations and their measurement quality;
these reservations explain capacity that is unavailable before terminal
reconciliation. The console does not estimate consumption from partial events.

The current `list` command calls the legacy unpaged API. It does not satisfy
the C5.8 bounded search/listing requirement. Do not use it as an inventory
export on an unbounded host; the paginated storage query and public opaque
cursor must land before that capability is accepted.
