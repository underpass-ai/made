//! Infrastructure adapters for MADE.
//!
//! Implements ports defined in `made-core` using concrete technologies.
//! MADE itself is **provider-agnostic**; every integration —
//! transport, message bus, or LLM vendor — is a peer adapter gated behind
//! its own Cargo feature flag. No provider is privileged.
//!
//! ## Always available
//!
//! | Adapter                             | Port                              |
//! |-------------------------------------|-----------------------------------|
//! | [`clock::SystemClock`]              | `ClockPort`                       |
//! | [`config::EnvConfiguration`]        | deployable configuration loader   |
//! | [`memory::InMemoryCouncilRegistry`] | `CouncilRegistryPort`             |
//! | [`memory::InMemoryDeliberationRepository`] | `DeliberationRepositoryPort` |
//! | [`memory::InMemoryAgentRegistry`]   | `AgentResolverPort` (+ writes)    |
//! | [`metrics::PrometheusMetricsRecorder`] | `MetricsRecorderPort`          |
//! | [`noop::NoopAgent`]                 | `AgentPort` (deterministic; tests / demos) |
//! | [`noop::NoopExecutor`]              | `ExecutorPort`                    |
//! | [`noop::NoopMessaging`]             | `MessagingPort`                   |
//! | [`scoring::UniformScoring`]         | `ScoringPort` (pass fraction)     |
//! | [`scoring::JudgeAwareScoring`]      | `ScoringPort` (judge verdict, wired when a judge is configured) |
//! | [`validators::ContentNonEmptyValidator`] | `ValidatorPort` (sanity check) |
//!
//! ## Feature-gated
//!
//! | Feature            | Integration                                 |
//! |--------------------|---------------------------------------------|
//! | `grpc` (default)   | Tonic gRPC server adapter (`grpc::*`)       |
//! | `nats`             | NATS JetStream messaging adapter            |
//! | `runtime-grpc`     | Outbound Underpass Runtime gRPC executor    |
//! | `postgres`         | Postgres deliberation repository (sqlx)     |
//! | `otel`             | gRPC W3C tracecontext → OpenTelemetry bridge |
//! | `agent-vllm`       | vLLM / OpenAI-compatible local inference    |
//! | `agent-anthropic`  | Anthropic Messages API                      |
//! | `agent-openai`     | OpenAI Chat Completions / Responses API     |
//!
//! Additional provider adapters (frontier, local, rule-based, human-in-the-loop)
//! plug in through the same `AgentPort` trait with no core changes.

#![deny(missing_debug_implementations)]

pub mod activation;
pub mod artifacts;
pub mod ceremony;
mod ceremony_event_wire;
pub mod clock;
pub mod config;
pub mod connectors;
mod delivery;
pub mod event_sink;
pub mod execution;
pub mod execution_profile_resolver;
pub mod json;
pub mod memory;
pub mod mermaid;
pub mod metrics;
pub mod noop;
pub mod progress;

mod process_group;
pub mod providers;
#[cfg(feature = "runtime-grpc")]
pub mod runtime;
pub mod scoring;
#[cfg(feature = "otel")]
pub mod telemetry;
pub mod validators;
pub mod workers;
pub mod yaml;

#[cfg(feature = "grpc")]
pub mod grpc;

/// Re-exports intended only for this crate's integration tests.
///
/// Hidden from rustdoc because no production consumer should depend
/// on these items — they exist so the integration harness under
/// `tests/` can reach helpers that are otherwise module-private.
#[cfg(all(feature = "grpc", feature = "otel"))]
#[doc(hidden)]
pub mod __test_only {
    pub use crate::grpc::tracecontext::link_span_to_metadata;
}

#[cfg(feature = "nats")]
pub mod nats;

#[cfg(feature = "postgres")]
pub mod postgres;

// The canonical embedded SQLite store and its internal storage seam.
// Nothing else in this crate needs them.
#[cfg(feature = "sqlite")]
pub(crate) mod engine;
#[cfg(feature = "sqlite")]
pub mod sqlite;

pub mod agents;

#[cfg(any(feature = "sqlite", feature = "postgres"))]
mod persisted_agent_descriptor;

pub mod council_journal_messaging;

mod stored_council_cursor;

#[cfg(any(feature = "sqlite", feature = "postgres"))]
pub mod council_data_snapshot;

#[cfg(any(feature = "sqlite", feature = "postgres"))]
mod council_snapshot_validation;
