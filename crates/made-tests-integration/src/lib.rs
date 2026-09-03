//! Integration tests for MADE's adapters.
//!
//! The actual scenarios live under `tests/`. The
//! `postgres_fixture` module is feature-gated behind
//! `container-tests` because spinning Postgres up requires Docker
//! and is expensive; the `grpc_fixture` module is always available
//! because it stands up an in-process gRPC server with in-memory
//! adapters and adds no external dependencies.

#[cfg(feature = "container-tests")]
pub mod postgres_fixture;

pub mod grpc_fixture;
pub mod tls_fixture;
mod tls_server_setup;
