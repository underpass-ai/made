//! Composition diagnostics kept separate from dependency wiring.

use made_adapters::config::ServiceConfig;

pub(super) fn wired(config: &ServiceConfig, agent_kinds: &str) {
    tracing::info!(
        grpc_port = config.grpc_port,
        http_port = config.http_port,
        nats_enabled = config.nats_enabled,
        executor_backend = super::executor::backend_name(),
        agent_kinds,
        trigger_subject = config.trigger_subject.as_str(),
        "made wired"
    );
}
