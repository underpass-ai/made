# made-adapters

Concrete integrations for [MADE](https://github.com/underpass-ai/made).

Implements domain ports for SQLite ceremony storage, YAML, gRPC, providers, messaging and service repositories. Features select optional integrations. The embedded composition uses this crate with default features disabled so its dependency graph does not pull in gRPC, NATS or Postgres clients.

[Documentation](https://github.com/underpass-ai/made/blob/main/docs/index.md) ·
[Architecture](https://github.com/underpass-ai/made/blob/main/docs/architecture/README.md) ·
[Migration notes](https://github.com/underpass-ai/made/blob/main/docs/migrations/README.md)

Apache-2.0. This crate follows the MADE workspace release. Refer to the
documentation at the matching tag when using a published version.
