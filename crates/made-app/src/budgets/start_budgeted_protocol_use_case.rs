use std::sync::Arc;

use made_core::entities::ProtocolInstance;
use made_core::ports::{ClockPort, MemoryReaderPort, ProtocolDefinitionPublicationPort};
use made_core::value_objects::BudgetAccountId;
use made_core::BudgetError;

use super::{BudgetLedgerService, StartBudgetedProtocolInput};
use crate::services::{memory_scope_resolver, session_facts, session_recall, SessionStream};

pub struct StartBudgetedProtocolUseCase {
    publications: Arc<dyn ProtocolDefinitionPublicationPort>,
    stream: Arc<SessionStream>,
    clock: Arc<dyn ClockPort>,
    memory: Arc<dyn MemoryReaderPort>,
    budgets: BudgetLedgerService,
}

impl std::fmt::Debug for StartBudgetedProtocolUseCase {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("StartBudgetedProtocolUseCase")
            .finish_non_exhaustive()
    }
}

impl StartBudgetedProtocolUseCase {
    #[must_use]
    pub fn new(
        publications: Arc<dyn ProtocolDefinitionPublicationPort>,
        stream: Arc<SessionStream>,
        clock: Arc<dyn ClockPort>,
        memory: Arc<dyn MemoryReaderPort>,
        budgets: BudgetLedgerService,
    ) -> Self {
        Self {
            publications,
            stream,
            clock,
            memory,
            budgets,
        }
    }

    pub async fn execute(
        &self,
        input: StartBudgetedProtocolInput,
    ) -> Result<ProtocolInstance, BudgetError> {
        let published = self
            .publications
            .published(
                &input.protocol.definition_name,
                &input.protocol.definition_version,
            )
            .await?
            .ok_or(made_core::DomainError::NotFound {
                what: "published_protocol_definition",
            })?;
        let actor =
            session_facts::party(input.protocol.actor_id.as_str(), input.protocol.actor_kind)?;
        let scope = memory_scope_resolver::of_context(&input.protocol.context, &input.protocol.id)?;
        let recalled = session_recall::recall(self.memory.as_ref(), &scope).await;
        let now = self.clock.now();
        let account_id = BudgetAccountId::for_root(&input.protocol.id)?;
        let opening = ProtocolInstance::decide_start_bound_budgeted(
            input.protocol.id.clone(),
            &published,
            input.protocol.context,
            account_id.clone(),
            recalled,
            now,
        )?;

        self.budgets.open(account_id, input.limits).await?;
        self.stream
            .open(opening, actor, now)
            .await
            .map(|session| session.instance)
            .map_err(BudgetError::from)
    }
}
