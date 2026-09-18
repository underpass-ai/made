# made-proto

Generated gRPC bindings for
[MADE by Underpass](https://github.com/underpass-ai/made), built from the
canonical `.proto` files at compile time by `tonic-build`.

Two wire contracts:

- **`underpass.made.v1`** — `MadeService`, the engine's own API:
  deliberations, councils, agents, output contracts and the full ceremony
  surface.
- **`underpass.runtime.v1`** — the client side of Underpass Runtime, used
  by the optional executor adapter.

The generated `v1` module is re-exported at the crate root.

## Which proto crate do I want?

This one is the workspace's internal binding, generated from
`crates/made-proto/proto/`, and it is what the server and the adapters
compile against.

If you are writing a client and only need to speak to MADE over gRPC,
[`made-mcp-proto`](https://crates.io/crates/made-mcp-proto) carries the
same `underpass.made.v1` contract with a smaller dependency footprint and
no runtime contract attached. The repository's contract gate diffs the two
copies on every change, so they cannot drift apart.

## License

Apache-2.0.
