//! Where work handed out to hosts is held, for the service.
//!
//! The same decision the ceremony store makes, made once more here and
//! from the same configuration: a durable stream beside an in-memory
//! ledger would restart into a service that had forgotten every
//! question it had asked a host.

use std::sync::Arc;

use made_adapters::activation::NoHostActivation;
use made_adapters::config::ServiceConfig;
use made_adapters::memory::{InMemoryHostDeliveryLedger, InMemoryIntegratorBindings};
use made_adapters::postgres::{
    PostgresHostDeliveryLedger, PostgresIntegratorBindings, PostgresPool,
};
use made_adapters::sqlite::{SqliteHostDeliveryLedger, SqliteIntegratorBindings};
use made_core::ports::HostActivationPort;
use tracing::{info, warn};

use crate::{ComposeError, HostDeliveryHandles};

/// Pick the delivery adapters that match the configured store.
///
/// Activation stays `none` here: the adapter that runs an operator's
/// command arrives with the loop that needs it, and a service that
/// claimed to wake hosts before it could would be lying in discovery.
pub(super) fn wire(
    config: &ServiceConfig,
    postgres: Option<&PostgresPool>,
) -> Result<HostDeliveryHandles, ComposeError> {
    let activation: Arc<dyn HostActivationPort> = Arc::new(NoHostActivation::new());
    if let Some(pool) = postgres {
        info!("host deliveries and integrator bindings are durable in Postgres");
        return Ok(HostDeliveryHandles {
            ledger: Arc::new(PostgresHostDeliveryLedger::new(pool.clone())),
            bindings: Arc::new(PostgresIntegratorBindings::new(pool.clone())),
            activation,
        });
    }
    let Some(path) = config.ceremony_store_path.as_deref() else {
        warn!(
            "MADE_CEREMONY_STORE_PATH is unset: work handed out to hosts is held in memory \
             and will not survive a restart"
        );
        return Ok(HostDeliveryHandles {
            ledger: Arc::new(InMemoryHostDeliveryLedger::new()),
            bindings: Arc::new(InMemoryIntegratorBindings::new()),
            activation,
        });
    };
    let ledger = SqliteHostDeliveryLedger::open(path).map_err(|error| {
        ComposeError::CeremonyStore(format!("host delivery ledger at {path}: {error}"))
    })?;
    let bindings = SqliteIntegratorBindings::open(path).map_err(|error| {
        ComposeError::CeremonyStore(format!("integrator bindings at {path}: {error}"))
    })?;
    info!(path, "host deliveries and integrator bindings are durable");
    Ok(HostDeliveryHandles {
        ledger: Arc::new(ledger),
        bindings: Arc::new(bindings),
        activation,
    })
}
