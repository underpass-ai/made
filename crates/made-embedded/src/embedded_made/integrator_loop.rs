//! The five verbs a host drives a whole scope by.
//!
//! Their own file rather than beside the intervention-delivery verbs
//! next door, because those four are about one question put to one
//! working agent and these five are about who is driving the scope at
//! all. They run over the loop this engine already composed rather
//! than building their own: a second wiring would read a ledger no
//! projection fills and answer every question with an empty queue.

use made_app::usecases::integrator::{
    AcknowledgeIntegratorAttentionInput, AttentionBatch, AwaitIntegratorAttentionInput,
    BindCeremonyIntegratorInput, IntegratorAttentionAcknowledged, ListAttentionDeliveriesInput,
};
use made_core::ports::{BindOutcome, HostDeliveryPage};
use made_core::value_objects::{AuthorizationAction, IntegratorBinding, IntegratorScope};
use made_core::DomainError;

use super::EmbeddedMade;

impl EmbeddedMade {
    /// Put one host in charge of one scope.
    pub async fn bind_integrator(
        &self,
        input: BindCeremonyIntegratorInput,
    ) -> Result<BindOutcome, DomainError> {
        self.require_authorized_integrator_scope(
            AuthorizationAction::BindCeremonyIntegrator,
            &input.scope,
        )?;
        self.integrator_loop().bind().execute(input).await
    }

    /// Who is driving a scope now, and under which fence.
    pub async fn get_integrator_binding(
        &self,
        scope: &IntegratorScope,
    ) -> Result<Option<IntegratorBinding>, DomainError> {
        self.require_authorized_integrator_scope(
            AuthorizationAction::GetCeremonyIntegratorBinding,
            scope,
        )?;
        self.integrator_loop().binding().execute(scope).await
    }

    /// Hand a bound host whatever it is owed, holding the line for a
    /// bounded while when there is nothing yet.
    pub async fn await_integrator_attention(
        &self,
        input: AwaitIntegratorAttentionInput,
    ) -> Result<AttentionBatch, DomainError> {
        self.require_authorized_integrator_scope(
            AuthorizationAction::AwaitIntegratorAttention,
            &input.scope,
        )?;
        self.integrator_loop()
            .await_attention()
            .execute(input)
            .await
    }

    /// Record what the host is about to do about one item, or what it
    /// did, or that it could not.
    ///
    /// Global, because an acknowledgement names a binding and a lease:
    /// resolving that binding to a ceremony here would authorize
    /// against whatever the ledger says rather than against what the
    /// caller asked for.
    pub async fn acknowledge_integrator_attention(
        &self,
        input: AcknowledgeIntegratorAttentionInput,
    ) -> Result<IntegratorAttentionAcknowledged, DomainError> {
        self.require_authorized_global_action(AuthorizationAction::AcknowledgeIntegratorAttention)?;
        self.integrator_loop().acknowledge().execute(input).await
    }

    /// The loop's paperwork, as an operator reads it.
    pub async fn list_attention_deliveries(
        &self,
        input: ListAttentionDeliveriesInput,
    ) -> Result<HostDeliveryPage, DomainError> {
        match input.ceremony_id.as_ref() {
            Some(ceremony_id) => self.require_authorized_ceremony_action(
                AuthorizationAction::ListAttentionDeliveries,
                ceremony_id,
            ),
            // Unnarrowed, the question is about the deployment rather
            // than about one session, and a ceremony grant does not
            // answer it.
            None => {
                self.require_authorized_global_action(AuthorizationAction::ListAttentionDeliveries)
            }
        }?;
        self.integrator_loop().deliveries().execute(input).await
    }

    /// A scope is a ceremony when it names one, and the host level when
    /// it names a run of a composed system instead (ADR-021).
    fn require_authorized_integrator_scope(
        &self,
        action: AuthorizationAction,
        scope: &IntegratorScope,
    ) -> Result<(), DomainError> {
        match scope.ceremony_id() {
            Some(ceremony_id) => self.require_authorized_ceremony_action(action, ceremony_id),
            None => self.require_authorized_global_action(action),
        }
    }
}
