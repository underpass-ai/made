//! Waking the host a delivery is addressed to.
//!
//! Which adapter a deployment has is an operator's decision, and the
//! honest default is none: hosts find their work by asking for it.

mod no_host_activation;

pub use no_host_activation::NoHostActivation;
