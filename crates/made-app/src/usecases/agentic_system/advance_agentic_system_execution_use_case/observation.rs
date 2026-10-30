//! What the instances actually say.
//!
//! A run never records progress it hoped for. Every status here is
//! read back from the instance's own folded state, and the input a
//! later protocol receives is taken from the context an earlier one
//! actually left behind.

use std::collections::BTreeMap;
use std::sync::Arc;

use made_core::entities::{AgenticSystem, AgenticSystemExecution, ProtocolInstance};
use made_core::error::DomainError;
use made_core::value_objects::{
    Attributes, LinkStatus, ProtocolComposition, ProtocolContext, ProtocolEndReason, ProtocolId,
    ProtocolLifecyclePhase, SystemProtocolId,
};

use crate::services::SessionStream;

/// Reads instances back through the same stream every other use case
/// folds.
pub struct Observation {
    stream: Arc<SessionStream>,
}

impl std::fmt::Debug for Observation {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("Observation").finish()
    }
}

impl Observation {
    #[must_use]
    pub const fn new(stream: Arc<SessionStream>) -> Self {
        Self { stream }
    }

    /// Bring every started link up to date with its instance.
    pub(super) async fn settle(
        &self,
        mut execution: AgenticSystemExecution,
        system: &AgenticSystem,
    ) -> Result<AgenticSystemExecution, DomainError> {
        let started: Vec<(SystemProtocolId, ProtocolId)> = execution
            .protocols()
            .iter()
            .filter(|(_, link)| link.status() == LinkStatus::Started)
            .filter_map(|(id, link)| {
                link.instance_id()
                    .map(|instance| (id.clone(), instance.clone()))
            })
            .collect();
        let _ = system;
        for (protocol, instance) in started {
            let Some(status) = self.status_of(&instance).await? else {
                continue;
            };
            let link = execution
                .link(&protocol)
                .ok_or(DomainError::NotFound {
                    what: "agentic_system_execution.protocol",
                })?
                .settled(status);
            let now = execution.updated_at();
            execution = execution.with_link(&protocol, link, now)?;
        }
        Ok(execution)
    }

    /// The context a composition should start with, projected from
    /// what the protocols it reads actually produced.
    ///
    /// An output that is not there is not projected. Inventing a value
    /// would let a protocol run on a premise nobody established, and
    /// the instance's own required-input check is a better place to
    /// refuse than a silent default here.
    pub(super) async fn projected_inputs(
        &self,
        execution: &AgenticSystemExecution,
        composition: &ProtocolComposition,
    ) -> Result<ProtocolContext, DomainError> {
        let mut entries: BTreeMap<String, serde_json::Value> = BTreeMap::new();
        for (input, source) in composition.inputs_from() {
            let Some(instance) = execution
                .link(source.protocol())
                .and_then(made_core::value_objects::ProtocolExecutionLink::instance_id)
            else {
                continue;
            };
            let Some(session) = self.load(instance).await? else {
                continue;
            };
            if let Some(value) = session.context().attributes().get(source.output().as_str()) {
                entries.insert(input.as_str().to_owned(), value.clone());
            }
        }
        Ok(ProtocolContext::new(Attributes::new(entries)?))
    }

    /// What one instance says about itself, for a view that shows
    /// intended beside observed.
    pub async fn instance(
        &self,
        instance: &ProtocolId,
    ) -> Result<Option<ProtocolInstance>, DomainError> {
        self.load(instance).await
    }

    async fn status_of(&self, instance: &ProtocolId) -> Result<Option<LinkStatus>, DomainError> {
        let Some(session) = self.load(instance).await? else {
            return Ok(None);
        };
        let lifecycle = session.lifecycle();
        if lifecycle.phase() != ProtocolLifecyclePhase::Ended {
            return Ok(None);
        }
        Ok(Some(match lifecycle.end_reason() {
            Some(ProtocolEndReason::Completed) => LinkStatus::Completed,
            // Cancelled or timed out. The work did not produce what it
            // promised, and whatever waited for it must not start.
            _ => LinkStatus::Failed,
        }))
    }

    /// An instance that is not there is not an error here.
    ///
    /// A link can name an instance a host has not opened yet, and a
    /// run that refused to be looked at because of it would be
    /// unreadable exactly when somebody most wanted to read it.
    async fn load(&self, instance: &ProtocolId) -> Result<Option<ProtocolInstance>, DomainError> {
        match self.stream.load(instance).await {
            Ok(session) => Ok(Some(session.instance)),
            Err(DomainError::NotFound { .. }) => Ok(None),
            Err(error) => Err(error),
        }
    }
}
