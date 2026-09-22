//! Waking the host a delivery is addressed to.
//!
//! Which adapter a deployment has is an operator's decision, and the
//! honest default is none: hosts find their work by asking for it. The
//! one alternative runs a command the operator configured, with the
//! envelope on its input and never as its instructions.

mod command_host_activation;
mod host_activation_command;
mod host_activation_config_error;
mod no_host_activation;
mod select_host_activation;

pub use command_host_activation::{
    CommandHostActivation, COMMAND_ENV, DELIVERY_ID_VAR, DESTINATION_VAR, HOST_KIND_VAR,
    MAX_OUTPUT_ENV, TIMEOUT_MS_ENV,
};
pub use host_activation_command::HostActivationCommand;
pub use host_activation_config_error::HostActivationConfigError;
pub use no_host_activation::NoHostActivation;
pub use select_host_activation::select_host_activation;
