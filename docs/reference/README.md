# API and MCP reference

Start by asking the running MCP server for `tools/list`. Then call
`made_discover_capabilities` for its backend, version, capability groups and
artifact generators. `made_get_help` returns guidance for `user` or `agent`.
These two MCP-owned tools have no corresponding gRPC RPC.

| Contract | Authority |
|:--|:--|
| gRPC service | [made.proto](../../crates/made-proto/proto/underpass/made/v1/made.proto) |
| NATS events and trigger subjects | [AsyncAPI](../../specs/asyncapi/made.asyncapi.yaml) |
| Cross-surface availability | [Parity ledger](../architecture/parity.tsv) and [support matrix](../operations/support-matrix.md) |
| Rust consumer interface | [CeremonyEngineApi](../../crates/made-api/src/ceremony_engine_api.rs) |
| Embedded host operations | [made-embedded](../../crates/made-embedded/README.md) |
| Number conversion | [Struct-number ledger](../architecture/struct-numbers.tsv) |

The vendored MCP protobuf file is packaging material for the same contract;
the contract gate compares it byte for byte with the service proto. It is not
an independently versioned API.

## Ceremony workflows

Design, validation, publication, execution, recovery, human decisions,
interventions, evidence, history and reports are available through the shared
ceremony surfaces, with specific facade exceptions listed in the ledger.
`made-api` deliberately exposes a smaller consumer subset; it is not a fifth
full transport. Check its `ApiCapabilities` and `CONTRACT_VERSION` rather than
assuming crate version implies every capability.

Council deliberation and council, agent and output-contract configuration are
available through gRPC, both MCP backends and `EmbeddedMade`. The embedded
builder supplies process-local in-memory registries by default and accepts
injected adapters. Explicit SQLite composition persists council records and
their independent journal, including leased consumer cursors. A provider also
requires its build feature, runtime configuration and registered agent kind.

## Recoverable execution

Receipt lookup and paged recovery inspection are shared public operations.
Completion applies a durable receipt to its producing claim; adoption applies
a recoverable receipt to the current replacement claim. Both require the
accepted claim fence, and verify linked artifact metadata against the artifact
store before appending the result. Retries preserve the semantic operation ID
and reuse its durable receipt. The [worker guide](../operations/recoverable-workers.md)
defines admission, external effects and unresolved reconciliation.

## Artifact transfer

Artifacts use eight matching gRPC, embedded facade and MCP operations: begin,
put chunk, commit, abort, get, list, read chunk and tombstone. A begin request
declares the canonical SHA-256 digest, exact byte size, MIME type, provenance
and idempotency key. Upload chunks carry their own digest and exact offset;
commit succeeds only after the declared size and whole-content digest match.
The default chunk is 64 KiB, each chunk is capped at 1 MiB, one artifact is
capped at 1 GiB, and metadata pages default to 50 with a maximum of 100.

List cursors are opaque, scoped and tamper-checked. An `ArtifactRef` contains
identity, digest, size, type and provenance, never a path, URI or blob. A
tombstone retains the audit record while content reads are refused. Actor
strings at this boundary are host assertions; the host must authorize them
until the C5.7 principal policy is applied. Backup and restore remain local
operator operations and are not exposed through gRPC or MCP.

The [runtime guide](../runtime/README.md) describes sequencing and identity.
Exact request fields belong to the installed schema, especially across the
post-v0.5.0 fence and visit changes.

## Errors and evidence

Invalid shape or missing required identity is a request error. A well-formed
command can still be refused by current state, a guard, a stale claim or a
concurrency conflict. Refresh state to decide the next action; do not interpret
an error as permission to skip a step. Retry an uncertain completion only
with the accepted claim's identity and the same observed result.

Reports return Markdown in `report_markdown` and declare `persisted: false`.
A host can then save those bytes through the artifact upload API with
`generated_report` provenance. Tool success, journal integrity and real
external work are separate claims; attach the evidence appropriate to each.

## Connect MCP to the service

Use a binary compiled with its `grpc` feature. Register this command and
environment with the host:

```bash
MADE_MCP_BACKEND=grpc \
MADE_MCP_GRPC_ENDPOINT=http://127.0.0.1:50055 made-mcp
```

The plugin launcher deliberately selects embedded mode, so use a separate
manual registration when choosing the service route. Avoid two active MADE
registrations in the same host. For server TLS set
`MADE_MCP_GRPC_TLS_MODE=server` and `MADE_MCP_GRPC_TLS_CA_PATH`; mutual TLS
also needs `MADE_MCP_GRPC_TLS_CERT_PATH` and `MADE_MCP_GRPC_TLS_KEY_PATH`.
`MADE_MCP_GRPC_TLS_DOMAIN_NAME` overrides the expected server name when
necessary. Client `disabled/server/mutual` names differ from the chart's
`none/server/mutual` names. See [backend configuration](../../crates/made-mcp/src/backend.rs)
for exact settings.

See [authorization in ceremony history](authorization-audit.md) for sealed
admission fields and independent verification of schema-3 records.
