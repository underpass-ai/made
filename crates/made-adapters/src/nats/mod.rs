//! NATS adapter.
//!
//! Two adapters live here: [`NatsMessaging`] publishes outbound
//! domain events, and [`NatsTriggerSubscriber`] consumes inbound
//! [`TriggerEvent`]s and hands them to the `AutoDispatchService`.
//!
//! Both honour the AsyncAPI contract in
//! `specs/asyncapi/made.asyncapi.yaml`: subjects under a
//! configurable prefix (default `made.*`), JSON payloads with the
//! envelope fields flattened at the root of each message.
//!
//! [`TriggerEvent`]: made_core::events::TriggerEvent

mod config;
mod messaging;
mod subscriber;

pub use config::{NatsConfig, NatsSubjects};
pub use messaging::NatsMessaging;
pub use subscriber::NatsTriggerSubscriber;
