# made-mcp-proto

Generated Rust types for the **`underpass.made.v1`** gRPC contract of
[MADE by Underpass](https://github.com/underpass-ai/made) — the
Multi-Agent Deliberation Engine.

This crate is a vendored copy of the contract, published so that
[`made-mcp`](https://crates.io/crates/made-mcp) — the stdio MCP adapter
coding agents talk to — resolves entirely from crates.io. The workspace
itself builds against its own `made-proto`; nothing internal depends on
this crate.

## What is in it

One thing: `MadeService`, generated at build time by `tonic-build` from
the `.proto` shipped inside the package. Thirty-five RPCs covering
deliberations (`Deliberate`, `StreamDeliberation`,
`GetDeliberationResult`), councils and agents, output contracts, and the
ceremony surface — publication, start, step claim and completion, guards,
transitions, interventions and evidence.

```rust
use made_mcp_proto::v1::{made_service_client::MadeServiceClient, DeliberateRequest};

let mut client = MadeServiceClient::connect("http://127.0.0.1:50055").await?;
let response = client.deliberate(DeliberateRequest { /* … */ }).await?;
```

The generated module is re-exported at the crate root, so
`made_mcp_proto::DeliberateRequest` and `made_mcp_proto::v1::DeliberateRequest`
are the same type.

## Two copies, one contract

A vendored contract can drift from the one the server actually serves,
and a drifted client fails in someone else's cluster rather than in CI.
It cannot drift here: the repository's contract gate diffs this `.proto`
against the canonical `crates/made-proto/proto/…` on every change and
fails when they differ, alongside `buf lint` and a breaking-change check
against `main`.

## Versions

The crate version tracks the MADE release it was cut from; the wire
contract is versioned separately and independently by its package name,
`underpass.made.v1`. Pair this crate with a `made-mcp` of the same
version.

Requires Rust 1.97. `prost` 0.13 and `tonic` 0.12 are pinned explicitly
rather than inherited, because a crate consumed by `cargo install`
outside the workspace cannot inherit workspace dependencies.

## License

Apache-2.0. See [`LICENSE`](https://github.com/underpass-ai/made/blob/main/LICENSE).
