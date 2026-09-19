use std::sync::Arc;

use made_core::entities::CeremonyInstance;
use made_core::ports::{CeremonyDefinitionPublicationPort, ClockPort, MemoryReaderPort};
use made_core::value_objects::BudgetAccountId;
use made_core::BudgetError;

use super::{BudgetLedgerService, StartBudgetedCeremonyInput};
use crate::services::{memory_scope_resolver, session_facts, session_recall, SessionStream};

pub struct StartBudgetedCeremonyUseCase {
    publications: Arc<dyn CeremonyDefinitionPublicationPort>,
    stream: Arc<SessionStream>,
    clock: Arc<dyn ClockPort>,
    memory: Arc<dyn MemoryReaderPort>,
    budgets: BudgetLedgerService,
}

impl std::fmt::Debug for StartBudgetedCeremonyUseCase {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("StartBudgetedCeremonyUseCase")
            .finish_non_exhaustive()
    }
}

impl StartBudgetedCeremonyUseCase {
    #[must_use]
    pub fn new(
        publications: Arc<dyn CeremonyDefinitionPublicationPort>,
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
        input: StartBudgetedCeremonyInput,
    ) -> Result<CeremonyInstance, BudgetError> {
        let published = self
            .publications
            .published(
                &input.ceremony.definition_name,
                &input.ceremony.definition_version,
            )
            .await?
            .ok_or(made_core::DomainError::NotFound {
                what: "published_ceremony_definition",
            })?;
        let actor =
            session_facts::party(input.ceremony.actor_id.as_str(), input.ceremony.actor_kind)?;
        let scope = memory_scope_resolver::of_context(&input.ceremony.context, &input.ceremony.id)?;
        let recalled = session_recall::recall(self.memory.as_ref(), &scope).await;
        let now = self.clock.now();
        let account_id = BudgetAccountId::for_root(&input.ceremony.id)?;
        let opening = CeremonyInstance::decide_start_bound_budgeted(
            input.ceremony.id.clone(),
            &published,
            input.ceremony.context,
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
