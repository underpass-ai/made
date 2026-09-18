# made-mcp-proto

Packaged MCP protobuf bindings for [MADE](https://github.com/underpass-ai/made).

Vendors the same service protobuf contract so the MCP crate can be distributed independently through Cargo. The contract gate compares this schema byte for byte with `made-proto`; there is no separate MCP-owned service schema.

[Documentation](https://github.com/underpass-ai/made/blob/main/docs/index.md) ·
[Architecture](https://github.com/underpass-ai/made/blob/main/docs/architecture/README.md) ·
[Migration notes](https://github.com/underpass-ai/made/blob/main/docs/migrations/README.md)

Apache-2.0. This crate follows the MADE workspace release. Refer to the
documentation at the matching tag when using a published version.
