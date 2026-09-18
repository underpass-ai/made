# made-api

Consumer Rust contract for [MADE](https://github.com/underpass-ai/made).

Exposes `CeremonyEngineApi`, plain views, capabilities and errors without domain or storage types. This is a deliberately smaller consumer boundary than the full embedded facade. Inspect `ApiCapabilities`; `CONTRACT_VERSION` tracks meaning independently of the Cargo release number.

[Documentation](https://github.com/underpass-ai/made/blob/main/docs/index.md) ·
[Architecture](https://github.com/underpass-ai/made/blob/main/docs/architecture/README.md) ·
[Migration notes](https://github.com/underpass-ai/made/blob/main/docs/migrations/README.md)

Apache-2.0. This crate follows the MADE workspace release. Refer to the
documentation at the matching tag when using a published version.
