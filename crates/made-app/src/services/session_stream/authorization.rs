use super::SessionStream;
use crate::authorization::AuthorizeCeremonyAppendUseCase;
use crate::services::current_authorized_operation;
use made_core::entities::AuditFact;
use made_core::ports::AppendOutcome;
use made_core::value_objects::{CeremonyId, StreamVersion};
use made_core::DomainError;
use std::sync::atomic::Ordering;
use std::sync::Arc;

impl SessionStream {
    /// Turn an already-composed embedded stream into a protected runtime
    /// boundary before it is shared with callers.
    pub fn require_authorization(&self) {
        self.authorization_required.store(true, Ordering::Release);
    }

    /// Install the policy services used by long-running protected operations.
    pub fn authorize_appends(&self, authorize: Arc<AuthorizeCeremonyAppendUseCase>) {
        *self
            .append_authorization
            .write()
            .expect("append authorization lock poisoned") = Some(authorize);
        self.require_authorization();
    }

    pub(super) async fn append(
        &self,
        ceremony_id: &CeremonyId,
        version: StreamVersion,
        mut facts: Vec<AuditFact>,
    ) -> Result<AppendOutcome, DomainError> {
        if let Some(operation) =
            Self::active_authorization(self.authorization_required.load(Ordering::Acquire))?
        {
            let authorize = self
                .append_authorization
                .read()
                .expect("append authorization lock poisoned")
                .clone();
            let operation = match authorize {
                Some(authorize) => {
                    Box::pin(authorize.execute(self, ceremony_id, version, &facts, operation))
                        .await?
                }
                None => operation,
            };
            // The domain event retains its observation time. The journal fact
            // is admitted now; never backdate its renewed authorization.
            for fact in &mut facts {
                fact.occurred_at = fact.occurred_at.max(operation.evidence().admitted_at());
            }
            return self
                .events
                .append_authorized(ceremony_id, version, facts, operation.evidence().clone())
                .await;
        }
        self.events.append(ceremony_id, version, facts).await
    }

    pub(crate) fn active_authorization(
        required: bool,
    ) -> Result<Option<made_core::value_objects::AuthorizedOperation>, DomainError> {
        let operation = current_authorized_operation();
        if required && operation.is_none() {
            return Err(DomainError::InvariantViolated {
                reason: "protected ceremony append has no active authorization",
            });
        }
        Ok(operation)
    }
}
