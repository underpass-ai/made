use made_adapters::postgres::PostgresPoolError;
use made_adapters::runtime::RuntimeExecutorConnectError;
use made_core::error::DomainError;
use thiserror::Error;

use crate::seeding::SeedingError;

/// Errors produced while composing the application.
#[derive(Debug, Error)]
pub enum ComposeError {
    #[error("domain error during wiring: {0}")]
    Domain(#[from] DomainError),

    #[error("nats connection failed: {0}")]
    NatsConnect(#[source] async_nats::ConnectError),

    #[error("postgres setup failed: {0}")]
    Postgres(#[from] PostgresPoolError),

    #[error("seeding failed: {0}")]
    Seeding(#[from] SeedingError),

    #[error("runtime executor setup failed: {0}")]
    RuntimeExecutor(#[from] RuntimeExecutorConnectError),

    #[error("ceremony store setup failed: {0}")]
    CeremonyStore(String),
}
