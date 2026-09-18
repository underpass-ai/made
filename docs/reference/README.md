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
injected adapters; its SQLite ceremony store does not persist those council
records. A provider also requires its build feature, runtime configuration and
registered agent kind.

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
Saving the report is a host action. Tool success, journal integrity and real
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
