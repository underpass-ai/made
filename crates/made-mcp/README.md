# made-mcp

![MADE — Your business. Your agents. Your architecture. — by Underpass](https://raw.githubusercontent.com/underpass-ai/made/main/crates/made-mcp/assets/made-cuatro-voces.png)

MADE over MCP stdio for [MADE](https://github.com/underpass-ai/made).

Exposes the executable tool catalogue over stdio with embedded and gRPC
backends. `cargo install made-mcp --version 0.9.0 --locked` includes both by
default. For local durable ceremonies, use the
[manual setup guide](https://github.com/underpass-ai/made/blob/main/docs/embedded/README.md#register-mcp):
embedded mode needs an absolute SQLite path, a bootstrapped authorization
policy, a trusted-host identity, a store id and a persistent search cursor key.
The plugin setup workflow configures these values automatically. Bootstrap
grants nothing: a person allows the ordinary route with
`made-mcp grant <store> --profile core` from their own terminal (see
[who may act](https://github.com/underpass-ai/made/blob/main/docs/embedded/README.md#who-may-act)).

`tools/list` and `made_discover_capabilities` describe the running build;
`made_get_help` provides user/agent guidance.

[Documentation](https://github.com/underpass-ai/made/blob/main/docs/index.md) ·
[Architecture](https://github.com/underpass-ai/made/blob/main/docs/architecture/README.md) ·
[Migration notes](https://github.com/underpass-ai/made/blob/main/docs/migrations/README.md)

Apache-2.0. This crate follows the MADE workspace release. Refer to the
documentation at the matching tag when using a published version.
