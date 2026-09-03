use serde::{de::DeserializeOwned, Serialize};

use crate::events::{
    DeliberationCompletedEvent, PhaseChangedEvent, TaskCompletedEvent, TaskDispatchedEvent,
    TaskFailedEvent,
};

/// Marker for serializable domain events accepted by messaging adapters.
pub trait DomainEvent: Serialize + DeserializeOwned + Send + Sync + 'static {}

impl DomainEvent for TaskDispatchedEvent {}
impl DomainEvent for TaskCompletedEvent {}
impl DomainEvent for TaskFailedEvent {}
impl DomainEvent for DeliberationCompletedEvent {}
impl DomainEvent for PhaseChangedEvent {}
