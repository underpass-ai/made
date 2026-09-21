//! Where work handed out to hosts is held, for the service.
//!
//! The same decision the ceremony store makes, made once more here and
//! from the same configuration: a durable stream beside an in-memory
//! ledger would restart into a service that had forgotten every
//! question it had asked a host.

use std::sync::Arc;

use made_adapters::activation::select_host_activation;
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
/// Activation is the operator's own decision, read from the environment
/// once: a service that claimed to wake hosts it cannot reach would be
/// lying in discovery, and one that quietly fell back to waking nobody
/// when its command was misspelled would leave a queue draining into
/// silence. Both answers are declared, and a broken one stops the boot.
pub(super) fn wire(
    config: &ServiceConfig,
    postgres: Option<&PostgresPool>,
) -> Result<HostDeliveryHandles, ComposeError> {
    let activation: Arc<dyn HostActivationPort> = select_host_activation()
        .map_err(|error| ComposeError::HostActivation(error.to_string()))?;
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
