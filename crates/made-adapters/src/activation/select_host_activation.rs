use std::sync::Arc;

use made_core::ports::HostActivationPort;

use super::{CommandHostActivation, HostActivationConfigError, NoHostActivation};

/// Which activation adapter this process composed, read once at startup.
///
/// One decision in one place, made the same way at every composition:
/// a service and an embedded host that disagreed about whether they
/// wake anybody would give a host two different answers to the same
/// question. A command that is configured and cannot be resolved stops
/// the process rather than degrading quietly into `none`: silently
/// falling back is how a deployment ends up watching a queue nobody is
/// draining.
///
/// # Errors
///
/// Returns the configuration failure when a command is configured and
/// cannot be used.
pub fn select_host_activation() -> Result<Arc<dyn HostActivationPort>, HostActivationConfigError> {
    Ok(CommandHostActivation::from_env()?.map_or_else(
        || Arc::new(NoHostActivation::new()) as Arc<dyn HostActivationPort>,
        |adapter| Arc::new(adapter) as Arc<dyn HostActivationPort>,
    ))
}
