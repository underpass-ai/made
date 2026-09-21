//! The integrator loop: binding a host to a scope, handing it what it
//! is owed, and hearing back what it did.
//!
//! Together in one module because they are one conversation. Every
//! other use case in this crate is a thing somebody asks MADE to do;
//! these five are the two halves of a round trip that keeps going on
//! its own, and reading them apart hides that the fence checked in one
//! is the fence raised by another.

mod acknowledge_integrator_attention_input;
mod acknowledge_integrator_attention_use_case;
mod attention_batch;
mod attention_context;
mod attention_delivery;
mod attention_end_reason;
mod await_integrator_attention_input;
mod await_integrator_attention_use_case;
mod bind_ceremony_integrator_input;
mod bind_ceremony_integrator_use_case;
mod get_ceremony_integrator_binding_use_case;
mod integrator_acknowledgement;
mod integrator_attention_acknowledged;
mod list_attention_deliveries_input;
mod list_attention_deliveries_use_case;

pub use acknowledge_integrator_attention_input::AcknowledgeIntegratorAttentionInput;
pub use acknowledge_integrator_attention_use_case::AcknowledgeIntegratorAttentionUseCase;
pub use attention_batch::AttentionBatch;
pub use attention_context::AttentionContext;
pub use attention_delivery::AttentionDelivery;
pub use attention_end_reason::AttentionEndReason;
pub use await_integrator_attention_input::{AwaitIntegratorAttentionInput, MAX_WAIT};
pub use await_integrator_attention_use_case::AwaitIntegratorAttentionUseCase;
pub use bind_ceremony_integrator_input::BindCeremonyIntegratorInput;
pub use bind_ceremony_integrator_use_case::BindCeremonyIntegratorUseCase;
pub use get_ceremony_integrator_binding_use_case::GetCeremonyIntegratorBindingUseCase;
pub use integrator_acknowledgement::IntegratorAcknowledgement;
pub use integrator_attention_acknowledged::IntegratorAttentionAcknowledged;
pub use list_attention_deliveries_input::ListAttentionDeliveriesInput;
pub use list_attention_deliveries_use_case::ListAttentionDeliveriesUseCase;
