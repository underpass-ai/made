# ADR-007: MCP self-description uses the executable catalog

Status: Accepted

## Context

MCP already exposes `tools/list`, but a caller that is choosing a workflow also
needs the running server version, backend, capability groups and artifact
generators. Human-facing documentation alone cannot answer that question: a
plugin bundle, a live gRPC backend and an embedded host may expose different
surfaces, and installed copies can lag the repository.

Static help has the same drift risk. A workflow that recommends an unavailable
tool is worse than no help because an autonomous host can mistake documentation
for executable capability. Report generation makes the boundary concrete: the
embedded backend can generate Markdown, while a backend without that operation
must not claim it or imply that the returned Markdown was persisted.

## Decision

The MCP server owns two backend-independent tools:

- `choreo_discover_capabilities` projects identity, backend metadata,
  capability groups, tools and artifact generators from the same
  backend-filtered catalog used by `tools/list`.
- `choreo_get_help` returns structured and Markdown guidance for either a
  `user` or an `agent`. Agent guidance includes preconditions, authority
  boundaries, the delegated-host sequence and error handling.

The backend continues to own domain and transport operations. Server-owned
tools are dispatched before the backend seam and are always present, while the
operations they describe remain filtered through `supports_tool`.

The discovery projection copies tool names, descriptions and input schemas
from the executable catalog rather than maintaining another list. Capability
groups and help workflows name tools, and coverage tests require every tool
reference returned for a backend to be advertised by that same backend.

Artifact generators are explicit records. The ceremony report generator names
its tool, artifact kind, media type and response field, and records
`persisted_by_tool: false` with the host as persistence owner.

## Consequences

- Agents can inspect the installed binary instead of assuming repository docs
  match it.
- User and agent help share the same availability boundary as execution.
- Adding a tool to a help workflow without making it executable fails coverage.
- Capability grouping and explanatory prose still require deliberate product
  maintenance; tests prevent dangling tool references but cannot judge prose.
- Discovery describes capability, not authorization. It does not grant a host
  permission to perform external work or convert an agent decision into human
  approval.
