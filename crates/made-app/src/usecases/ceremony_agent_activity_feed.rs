use std::sync::Arc;

use made_core::error::DomainError;
use made_core::ports::{
    CeremonyAgentActivityPage, CeremonyAgentActivityQuery, CeremonyAgentActivitySubscriptionPort,
    CeremonyAgentStatusPort,
};
use made_core::value_objects::AuthorizationEvidence;
use tokio::sync::mpsc;

use super::{CeremonyAgentSnapshot, CeremonyProgressFrame, StreamCeremonyInput};

pub(super) struct CeremonyAgentActivityFeed {
    port: Option<Arc<dyn CeremonyAgentStatusPort>>,
    subscription: Option<Box<dyn CeremonyAgentActivitySubscriptionPort>>,
    query: Option<CeremonyAgentActivityQuery>,
    authorization: Option<AuthorizationEvidence>,
    initial: Option<CeremonyAgentActivityPage>,
    frontier: u64,
    head: u64,
}

impl CeremonyAgentActivityFeed {
    pub(super) async fn open(
        input: &StreamCeremonyInput,
        port: Option<Arc<dyn CeremonyAgentStatusPort>>,
        authorization: Option<AuthorizationEvidence>,
    ) -> Result<Self, DomainError> {
        if !input.includes_agent_activity() {
            return Ok(Self::disabled());
        }
        let port = port.ok_or(DomainError::InvariantViolated {
            reason: "agent activity streaming is not configured",
        })?;
        let subscription = port.subscribe_activity();
        let query = activity_query(input, input.after_activity_sequence().value())?;
        let initial = port
            .read_activity(query.clone(), authorization.clone())
            .await?;
        let head = initial.head_sequence();
        Ok(Self {
            port: Some(port),
            subscription: Some(subscription),
            query: Some(query),
            authorization,
            initial: Some(initial),
            frontier: input.after_activity_sequence().value(),
            head,
        })
    }

    fn disabled() -> Self {
        Self {
            port: None,
            subscription: None,
            query: None,
            authorization: None,
            initial: None,
            frontier: 0,
            head: 0,
        }
    }

    pub(super) async fn deliver_initial(
        &mut self,
        sender: &mpsc::Sender<Result<CeremonyProgressFrame, DomainError>>,
        remaining: &mut usize,
    ) -> bool {
        let Some(page) = self.initial.take() else {
            return true;
        };
        if self.frontier == 0
            && sender
                .send(Ok(CeremonyProgressFrame::AgentSnapshot(
                    CeremonyAgentSnapshot::new(
                        page.snapshot().to_vec(),
                        page.head_sequence(),
                        page.snapshot_complete(),
                    ),
                )))
                .await
                .is_err()
        {
            return false;
        }
        self.deliver_page(sender, page, remaining).await
    }

    pub(super) async fn deliver_next(
        &mut self,
        sender: &mpsc::Sender<Result<CeremonyProgressFrame, DomainError>>,
        remaining: &mut usize,
    ) -> bool {
        let (Some(port), Some(query)) = (&self.port, &self.query) else {
            return true;
        };
        let query = match CeremonyAgentActivityQuery::new(
            query.ceremony_id().clone(),
            self.frontier,
            (*remaining).max(1),
            query.role_id().cloned(),
            query.step_id().cloned(),
            query.agent_execution_id().cloned(),
        ) {
            Ok(query) => query,
            Err(error) => {
                let _ignored = sender.send(Err(error)).await;
                return false;
            }
        };
        match port.read_activity(query, self.authorization.clone()).await {
            Ok(page) => self.deliver_page(sender, page, remaining).await,
            Err(error) => {
                let _ignored = sender.send(Err(error)).await;
                false
            }
        }
    }

    async fn deliver_page(
        &mut self,
        sender: &mpsc::Sender<Result<CeremonyProgressFrame, DomainError>>,
        page: CeremonyAgentActivityPage,
        remaining: &mut usize,
    ) -> bool {
        self.head = page.head_sequence();
        for activity in page.activities() {
            if *remaining == 0 {
                break;
            }
            if sender
                .send(Ok(CeremonyProgressFrame::AgentActivity(Box::new(
                    activity.clone(),
                ))))
                .await
                .is_err()
            {
                return false;
            }
            *remaining -= 1;
        }
        self.frontier = page.next_sequence();
        true
    }

    pub(super) async fn wait(&mut self) {
        match self.subscription.as_mut() {
            Some(subscription) => subscription.wait().await,
            None => std::future::pending::<()>().await,
        }
    }

    pub(super) const fn caught_up(&self) -> bool {
        self.frontier >= self.head
    }
    pub(super) const fn frontier(&self) -> u64 {
        self.frontier
    }
    pub(super) const fn head(&self) -> u64 {
        self.head
    }
}

fn activity_query(
    input: &StreamCeremonyInput,
    after_sequence: u64,
) -> Result<CeremonyAgentActivityQuery, DomainError> {
    CeremonyAgentActivityQuery::new(
        input.ceremony_id().clone(),
        after_sequence,
        input.max_events().value(),
        input.role_id().cloned(),
        input.step_id().cloned(),
        input.agent_execution_id().cloned(),
    )
}
