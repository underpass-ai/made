# made-mcp

![MADE — Your business. Your agents. Your architecture. — by Underpass](https://raw.githubusercontent.com/underpass-ai/made/main/crates/made-mcp/assets/made-cuatro-voces.png)

MADE over MCP stdio for [MADE](https://github.com/underpass-ai/made).

Exposes the executable tool catalogue over stdio with embedded and gRPC backends. `cargo install made-mcp --locked` includes both by default. Set `MADE_MCP_BACKEND=embedded` and an absolute `MADE_MCP_STORE_PATH` for local durable ceremonies. `tools/list` and `made_discover_capabilities` describe this running build; `made_get_help` provides user/agent guidance.

[Documentation](https://github.com/underpass-ai/made/blob/main/docs/index.md) ·
[Architecture](https://github.com/underpass-ai/made/blob/main/docs/architecture/README.md) ·
[Migration notes](https://github.com/underpass-ai/made/blob/main/docs/migrations/README.md)

Apache-2.0. This crate follows the MADE workspace release. Refer to the
documentation at the matching tag when using a published version.
