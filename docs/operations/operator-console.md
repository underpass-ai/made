# Operator console

The `made-console` binary reads and acts through MADE's public gRPC contract.
It never opens SQLite, Postgres, artifact paths or internal service endpoints.
Its reusable transport lives in `made-client`.

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
resume, cancel, deadline enforcement and guard approval. MADE records the
supplied actor or role, but the host remains responsible for authentication
and authorization until the C5.7 boundary is installed.

Budget is shown as unknown until the C5.3 public budget report is available.
The console does not estimate consumption from partial events.

The current `list` command calls the legacy unpaged API. It does not satisfy
the C5.8 bounded search/listing requirement. Do not use it as an inventory
export on an unbounded host; the paginated storage query and public opaque
cursor must land before that capability is accepted.
