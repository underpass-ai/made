//! [`CeremonyInstance`] aggregate.
//!
//! Runtime state for a single ceremony execution. The aggregate owns
//! step leases, retry attempts, idempotency keys and state transitions,
//! so failover remains a domain rule instead of adapter glue.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use super::{
    ceremony_definition::CeremonyDefinition, CeremonyEvidencePack, CeremonyIntervention,
    PublishedCeremonyDefinition,
};
use crate::error::DomainError;
use crate::ports::CeremonyEvidenceRequest;
use crate::value_objects::{
    AuditActorKind, CeremonyContext, CeremonyDefinitionDigest, CeremonyDefinitionDigestMigration,
    CeremonyEvidenceSourceId, CeremonyGuardApproval, CeremonyGuardDeferral,
    CeremonyGuardDeferralContent, CeremonyId, CeremonyInterventionContent, CeremonyInterventionId,
    CeremonyInterventionKind, CeremonyInterventionProvenance, CeremonyInterventionResponse,
    CeremonyInterventionTarget, CeremonyName, CeremonyParticipantBinding, CeremonyReason,
    CeremonyReasonKind, CeremonyRecordRef, CeremonyTransitionRecord, CeremonyVersion,
    GuardCondition, GuardName, IdempotencyKey, MemoryConfidence, ReasonAsserter, RoleAction,
    RoleId, Specialty, StateId, StepAttempt, StepExecutionRecord, StepId, StepLease, StepResult,
    StepStatus, TransitionTrigger,
};

mod guard_decisions;
mod interventions;
mod invariants;
mod participant_bindings;
mod step_execution;
mod transitions;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CeremonyInstance {
    id: CeremonyId,
    definition_name: CeremonyName,
    definition_version: CeremonyVersion,
    current_state: StateId,
    step_records: BTreeMap<StepId, StepExecutionRecord>,
    /// Finished semantic iterations preceding each step's current record.
    ///
    /// Technical retries remain represented by their attempt number and the
    /// audit journal. Semantic repetition needs its own durable history: one
    /// successful iteration must not overwrite the output that made MADE run
    /// the next one.
    #[serde(default)]
    step_record_history: BTreeMap<StepId, Vec<StepExecutionRecord>>,
    #[serde(default)]
    interventions: Vec<CeremonyIntervention>,
    #[serde(default)]
    guard_deferrals: Vec<CeremonyGuardDeferral>,
    /// Who let each human guard through.
    ///
    /// Kept beside the context rather than inside it. The context is
    /// what a guard is evaluated against and already holds "this one
    /// is approved"; sessions written before this existed carry that
    /// and nothing else, and moving approval out of the context would
    /// have made every one of them unapproved on the next read. So the
    /// context stays the state, and this is the event that produced
    /// it.
    #[serde(default)]
    guard_approvals: Vec<CeremonyGuardApproval>,
    /// Every move this session made, in order.
    ///
    /// The current state says where a session is; this says how it got
    /// there. Without it nothing could point at a move, so nothing
    /// could say why one happened — and "why did this resolve" is the
    /// question the whole thing is for.
    #[serde(default)]
    transitions: Vec<CeremonyTransitionRecord>,
    /// Why one thing here led to another.
    ///
    /// Kept apart from the records rather than inside them, because a
    /// reason is an edge and not a field: it belongs to the pair, and
    /// putting it on either end would make it readable but not
    /// followable.
    #[serde(default)]
    reasons: Vec<CeremonyReason>,
    /// Who sits in each seat for this session, where anyone was
    /// seated. A role with no binding is played the way the definition
    /// says, which is the usual case and not a lesser one.
    #[serde(default)]
    participant_bindings: BTreeMap<RoleId, CeremonyParticipantBinding>,
    context: CeremonyContext,
    idempotency_keys: BTreeSet<IdempotencyKey>,
    #[serde(with = "time::serde::rfc3339")]
    created_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    updated_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339::option")]
    completed_at: Option<OffsetDateTime>,
    /// The published definition this instance is bound to, when it was
    /// started from one.
    ///
    /// Absent for an instance started from a definition handed in at
    /// the time — which is a real and useful way to work, and not the
    /// same thing. Recording which of the two happened is the point: a
    /// name and a version identify a published definition only while
    /// publication is immutable, and an instance that also carries the
    /// digest can be checked against the definition rather than trusted
    /// to have run it.
    #[serde(default)]
    bound_definition: Option<CeremonyDefinitionDigest>,
}

impl CeremonyInstance {
    /// Start from a definition supplied for this run.
    ///
    /// Nothing binds the instance to a definition that can be looked up
    /// later; that is what [`Self::start_bound`] is for.
    #[must_use]
    pub fn start(
        id: CeremonyId,
        definition: &CeremonyDefinition,
        context: CeremonyContext,
        now: OffsetDateTime,
    ) -> Self {
        Self::open(id, definition, context, now, None)
    }

    /// Start from a published definition, recording its digest.
    ///
    /// The digest travels with the instance so a later reader can
    /// verify which definition ran instead of taking the name and
    /// version on trust.
    #[must_use]
    pub fn start_bound(
        id: CeremonyId,
        published: &PublishedCeremonyDefinition,
        context: CeremonyContext,
        now: OffsetDateTime,
    ) -> Self {
        Self::open(
            id,
            published.definition(),
            context,
            now,
            Some(published.digest()),
        )
    }

    fn open(
        id: CeremonyId,
        definition: &CeremonyDefinition,
        context: CeremonyContext,
        now: OffsetDateTime,
        bound_definition: Option<CeremonyDefinitionDigest>,
    ) -> Self {
        let step_records = definition
            .steps()
            .keys()
            .map(|step_id| (step_id.clone(), StepExecutionRecord::pending()))
            .collect();

        Self {
            id,
            definition_name: definition.name().clone(),
            definition_version: definition.version().clone(),
            current_state: definition.initial_state_id().clone(),
            step_records,
            step_record_history: BTreeMap::new(),
            interventions: Vec::new(),
            guard_deferrals: Vec::new(),
            guard_approvals: Vec::new(),
            transitions: Vec::new(),
            reasons: Vec::new(),
            participant_bindings: BTreeMap::new(),
            context,
            idempotency_keys: BTreeSet::new(),
            created_at: now,
            updated_at: now,
            completed_at: None,
            bound_definition,
        }
    }

    /// The digest of the published definition this instance runs, if it
    /// was started from one.
    #[must_use]
    pub fn bound_definition(&self) -> Option<CeremonyDefinitionDigest> {
        self.bound_definition
    }

    /// Whether this instance runs a definition that can be looked up
    /// and checked, rather than one supplied for the run.
    #[must_use]
    pub fn is_bound_to_a_published_definition(&self) -> bool {
        self.bound_definition.is_some()
    }

    /// Replace a legacy publication identity after the same definition has
    /// been verified under a successor digest scheme.
    ///
    /// This is deliberately narrower than a general rebind operation. A
    /// running ceremony cannot be moved to different content, name or
    /// version. Storage migrations may only replace the expected legacy
    /// identity with the identity of the already verified publication.
    pub fn migrate_definition_binding(
        &mut self,
        migration: &CeremonyDefinitionDigestMigration,
    ) -> Result<bool, DomainError> {
        if self.definition_name != *migration.definition_name()
            || self.definition_version != *migration.definition_version()
        {
            return Err(DomainError::InvariantViolated {
                reason: "a definition binding migration cannot change name or version",
            });
        }

        match self.bound_definition {
            Some(current) if current == migration.destination() => Ok(false),
            Some(current) if current == migration.source() => {
                self.bound_definition = Some(migration.destination());
                Ok(true)
            }
            _ => Err(DomainError::InvariantViolated {
                reason: "a definition binding migration did not match the stored identity",
            }),
        }
    }

    #[must_use]
    pub fn id(&self) -> &CeremonyId {
        &self.id
    }

    #[must_use]
    pub fn definition_name(&self) -> &CeremonyName {
        &self.definition_name
    }

    #[must_use]
    pub fn definition_version(&self) -> &CeremonyVersion {
        &self.definition_version
    }

    #[must_use]
    pub fn current_state(&self) -> &StateId {
        &self.current_state
    }

    #[must_use]
    pub fn step_records(&self) -> &BTreeMap<StepId, StepExecutionRecord> {
        &self.step_records
    }

    #[must_use]
    pub fn step_record(&self, step_id: &StepId) -> Option<&StepExecutionRecord> {
        self.step_records.get(step_id)
    }

    /// Finished iterations before the current record, in execution order.
    #[must_use]
    pub fn step_record_history(&self, step_id: &StepId) -> &[StepExecutionRecord] {
        self.step_record_history
            .get(step_id)
            .map(Vec::as_slice)
            .unwrap_or_default()
    }

    /// Whether a repeating step consumed its last permitted iteration without
    /// satisfying its declared stop condition.
    #[must_use]
    pub fn step_repeat_limit_reached(
        &self,
        definition: &CeremonyDefinition,
        step_id: &StepId,
    ) -> bool {
        let Some(step) = definition.step(step_id) else {
            return false;
        };
        let Some(policy) = step.repeat_policy() else {
            return false;
        };
        let Some(record) = self.step_record(step_id) else {
            return false;
        };
        record.status().is_success()
            && !policy.is_satisfied(record.output())
            && !policy.permits_another_iteration(record.iteration())
    }

    #[must_use]
    pub fn interventions(&self) -> &[CeremonyIntervention] {
        &self.interventions
    }

    #[must_use]
    pub fn guard_deferrals(&self) -> &[CeremonyGuardDeferral] {
        &self.guard_deferrals
    }

    /// Who let each human guard through, in the order they did.
    ///
    /// Empty for a session written before approvals recorded an
    /// approver, which is the truth about those sessions rather than a
    /// gap to paper over.
    #[must_use]
    pub fn guard_approvals(&self) -> &[CeremonyGuardApproval] {
        &self.guard_approvals
    }

    /// Every move this session made, in the order it made them.
    #[must_use]
    pub fn transitions(&self) -> &[CeremonyTransitionRecord] {
        &self.transitions
    }

    /// Why one thing here led to another.
    #[must_use]
    pub fn reasons(&self) -> &[CeremonyReason] {
        &self.reasons
    }

    #[must_use]
    pub fn intervention(
        &self,
        intervention_id: &CeremonyInterventionId,
    ) -> Option<&CeremonyIntervention> {
        self.interventions
            .iter()
            .find(|intervention| intervention.id() == intervention_id)
    }

    #[must_use]
    pub fn context(&self) -> &CeremonyContext {
        &self.context
    }

    #[must_use]
    pub fn idempotency_keys(&self) -> &BTreeSet<IdempotencyKey> {
        &self.idempotency_keys
    }

    #[must_use]
    pub fn created_at(&self) -> OffsetDateTime {
        self.created_at
    }

    #[must_use]
    pub fn updated_at(&self) -> OffsetDateTime {
        self.updated_at
    }

    #[must_use]
    pub fn completed_at(&self) -> Option<OffsetDateTime> {
        self.completed_at
    }

    #[must_use]
    pub fn is_terminal(&self, definition: &CeremonyDefinition) -> bool {
        self.matches_definition(definition) && definition.is_terminal_state(&self.current_state)
    }

    #[must_use]
    pub fn is_completed(&self, definition: &CeremonyDefinition) -> bool {
        self.is_terminal(definition) && self.completed_at.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value_objects::{
        Attributes, CeremonyGuard, CeremonyState, CeremonyStep, CeremonyTransition, GuardCondition,
        GuardName, LeaseOwnerId, RepeatUntilCondition, RetryPolicy, StepHandlerConfig,
        StepHandlerKind, StepIteration, StepOutput, StepOutputField, StepRepeatPolicy,
    };
    use serde_json::json;
    use time::macros::datetime;

    fn now() -> OffsetDateTime {
        datetime!(2026-06-06 12:00:00 UTC)
    }

    fn state_id(raw: &str) -> StateId {
        StateId::new(raw).unwrap()
    }

    fn step_id(raw: &str) -> StepId {
        StepId::new(raw).unwrap()
    }

    fn trigger(raw: &str) -> TransitionTrigger {
        TransitionTrigger::new(raw).unwrap()
    }

    fn role_id(raw: &str) -> RoleId {
        RoleId::new(raw).unwrap()
    }

    fn guard_name(raw: &str) -> GuardName {
        GuardName::new(raw).unwrap()
    }

    fn handler_kind() -> StepHandlerKind {
        StepHandlerKind::new("multiagent_round").unwrap()
    }

    fn retrying_step(raw_step_id: &str, raw_state_id: &str) -> CeremonyStep {
        CeremonyStep::new(
            step_id(raw_step_id),
            state_id(raw_state_id),
            handler_kind(),
            StepHandlerConfig::empty(),
            RetryPolicy::new(
                StepAttempt::new(3).unwrap(),
                crate::value_objects::DurationMs::ZERO,
            ),
            None,
        )
    }

    fn single_attempt_step(raw_step_id: &str, raw_state_id: &str) -> CeremonyStep {
        CeremonyStep::new(
            step_id(raw_step_id),
            state_id(raw_state_id),
            handler_kind(),
            StepHandlerConfig::empty(),
            RetryPolicy::single_attempt(),
            None,
        )
    }

    fn repeating_plan(max_iterations: u32) -> CeremonyStep {
        retrying_step("plan", "drafting").with_repeat_policy(StepRepeatPolicy::new(
            RepeatUntilCondition::output_field_equals(
                StepOutputField::new("ready").unwrap(),
                json!(true),
            ),
            StepIteration::new(max_iterations).unwrap(),
        ))
    }

    fn readiness_output(ready: bool) -> StepOutput {
        StepOutput::new(
            Attributes::new(std::collections::BTreeMap::from([(
                "ready".to_owned(),
                json!(ready),
            )]))
            .unwrap(),
        )
    }

    fn lease(
        raw_owner_id: &str,
        raw_key: &str,
        acquired_at: OffsetDateTime,
        expires_at: OffsetDateTime,
    ) -> StepLease {
        StepLease::new(
            LeaseOwnerId::new(raw_owner_id).unwrap(),
            IdempotencyKey::new(raw_key).unwrap(),
            acquired_at,
            expires_at,
        )
        .unwrap()
    }

    fn role(actions: Vec<RoleAction>) -> crate::value_objects::CeremonyRole {
        crate::value_objects::CeremonyRole::new(role_id("facilitator"), actions).unwrap()
    }

    fn definition_with_steps(steps: Vec<CeremonyStep>) -> CeremonyDefinition {
        let plan_done = CeremonyGuard::new(
            guard_name("plan_done"),
            GuardCondition::StepStatus {
                step_id: step_id("plan"),
                status: StepStatus::Completed,
            },
        );
        let finish = CeremonyTransition::new(
            state_id("drafting"),
            state_id("done"),
            trigger("finish"),
            vec![plan_done.name().clone()],
        )
        .unwrap();
        let role = role(vec![
            RoleAction::step(step_id("plan")),
            RoleAction::transition(finish.trigger().clone()),
            RoleAction::request_intervention(),
        ]);
        let observer = crate::value_objects::CeremonyRole::new(
            role_id("observer"),
            vec![RoleAction::respond_to_intervention()],
        )
        .unwrap();

        CeremonyDefinition::new(
            crate::value_objects::CeremonyName::new("planning_ceremony").unwrap(),
            CeremonyVersion::v1(),
            None,
            Vec::new(),
            Vec::new(),
            vec![
                CeremonyState::initial(state_id("drafting")),
                CeremonyState::intermediate(state_id("review")),
                CeremonyState::terminal(state_id("done")),
            ],
            vec![finish],
            steps,
            vec![plan_done],
            vec![role, observer],
        )
        .unwrap()
    }

    fn definition() -> CeremonyDefinition {
        definition_with_steps(vec![
            retrying_step("plan", "drafting"),
            single_attempt_step("review_step", "review"),
        ])
    }

    #[test]
    fn a_verified_digest_migration_rebinds_only_its_exact_definition() {
        let definition = definition();
        let published = PublishedCeremonyDefinition::seal(definition.clone()).unwrap();
        let migration = definition.choreographer_v1_digest_migration().unwrap();
        let mut value = serde_json::to_value(CeremonyInstance::start_bound(
            CeremonyId::new("legacy-bound").unwrap(),
            &published,
            CeremonyContext::empty(),
            now(),
        ))
        .unwrap();
        value["bound_definition"] = serde_json::to_value(migration.source()).unwrap();
        let mut instance: CeremonyInstance = serde_json::from_value(value).unwrap();

        assert!(instance.migrate_definition_binding(&migration).unwrap());
        assert_eq!(instance.bound_definition(), Some(migration.destination()));
        assert!(!instance.migrate_definition_binding(&migration).unwrap());
    }

    #[test]
    fn a_digest_migration_for_another_definition_is_rejected() {
        let definition = definition();
        let published = PublishedCeremonyDefinition::seal(definition.clone()).unwrap();
        let mut instance = CeremonyInstance::start_bound(
            CeremonyId::new("still-bound").unwrap(),
            &published,
            CeremonyContext::empty(),
            now(),
        );
        let other = CeremonyDefinition::new(
            CeremonyName::new("another_ceremony").unwrap(),
            CeremonyVersion::v1(),
            None,
            [],
            [],
            [CeremonyState::initial(state_id("OPEN"))],
            [],
            [],
            [],
            [],
        )
        .unwrap()
        .choreographer_v1_digest_migration()
        .unwrap();

        assert!(instance.migrate_definition_binding(&other).is_err());
        assert_eq!(instance.bound_definition(), Some(published.digest()));
    }

    /// The smallest ceremony that waits on a person: one guard, one
    /// transition it blocks, one seat allowed to fire it.
    fn definition_with_human_guard(approval: &CeremonyGuard) -> CeremonyDefinition {
        let finish = CeremonyTransition::new(
            state_id("drafting"),
            state_id("done"),
            trigger("approve"),
            vec![approval.name().clone()],
        )
        .unwrap();
        CeremonyDefinition::new(
            crate::value_objects::CeremonyName::new("approval_ceremony").unwrap(),
            CeremonyVersion::v1(),
            None,
            Vec::new(),
            Vec::new(),
            vec![
                CeremonyState::initial(state_id("drafting")),
                CeremonyState::terminal(state_id("done")),
            ],
            vec![finish.clone()],
            Vec::new(),
            vec![approval.clone()],
            vec![role(vec![RoleAction::transition(finish.trigger().clone())])],
        )
        .unwrap()
    }

    fn instance(definition: &CeremonyDefinition) -> CeremonyInstance {
        CeremonyInstance::start(
            CeremonyId::new("ceremony-1").unwrap(),
            definition,
            CeremonyContext::empty(),
            now(),
        )
    }

    #[test]
    fn starts_in_initial_state_with_pending_records() {
        let definition = definition();
        let instance = instance(&definition);

        assert_eq!(instance.current_state(), &state_id("drafting"));
        assert_eq!(
            instance.step_record(&step_id("plan")).unwrap().status(),
            StepStatus::Pending
        );
        assert_eq!(
            instance
                .step_record(&step_id("review_step"))
                .unwrap()
                .status(),
            StepStatus::Pending
        );
    }

    #[test]
    fn instances_without_iteration_fields_load_as_the_first_iteration() {
        let definition = definition();
        let mut value = serde_json::to_value(instance(&definition)).unwrap();
        value.as_object_mut().unwrap().remove("step_record_history");
        for record in value["step_records"].as_object_mut().unwrap().values_mut() {
            record.as_object_mut().unwrap().remove("iteration");
        }

        let restored: CeremonyInstance = serde_json::from_value(value).unwrap();

        assert!(restored.step_record_history(&step_id("plan")).is_empty());
        assert_eq!(
            restored.step_record(&step_id("plan")).unwrap().iteration(),
            StepIteration::FIRST
        );
    }

    #[test]
    fn dynamic_intervention_collects_role_scoped_response_and_requester_closes_it() {
        let definition = definition();
        let mut instance = instance(&definition);
        let intervention_id = CeremonyInterventionId::new("queue-check").unwrap();
        let facilitator = role_id("facilitator");
        let observer = role_id("observer");

        instance
            .request_intervention_as(
                &definition,
                intervention_id.clone(),
                facilitator.clone(),
                CeremonyInterventionKind::Investigation,
                CeremonyInterventionTarget::roles([observer.clone()]).unwrap(),
                CeremonyInterventionContent::new(
                    "Inspect the queue without consuming messages.",
                    Attributes::empty(),
                )
                .unwrap(),
                now(),
            )
            .unwrap();
        instance
            .respond_to_intervention_as(
                &definition,
                &intervention_id,
                observer.clone(),
                CeremonyInterventionContent::new("Queue depth is stable.", Attributes::empty())
                    .unwrap(),
                now(),
            )
            .unwrap();
        let selected_intervention_id = CeremonyInterventionId::new("selected-check").unwrap();
        instance
            .request_intervention_with_provenance_as(
                &definition,
                selected_intervention_id.clone(),
                facilitator.clone(),
                CeremonyInterventionKind::Investigation,
                CeremonyInterventionTarget::roles([observer.clone()]).unwrap(),
                CeremonyInterventionContent::new(
                    "Inspect the proposed signal.",
                    Attributes::empty(),
                )
                .unwrap(),
                Some(CeremonyInterventionProvenance::selected_from(
                    intervention_id.clone(),
                    observer.clone(),
                    observer.clone(),
                )),
                now(),
            )
            .unwrap();
        instance
            .close_intervention_as(&definition, &intervention_id, &facilitator, now())
            .unwrap();

        let intervention = instance.intervention(&intervention_id).unwrap();
        assert_eq!(intervention.responses().len(), 1);
        assert_eq!(
            intervention.status(),
            crate::value_objects::CeremonyInterventionStatus::Closed
        );
        let provenance = instance
            .intervention(&selected_intervention_id)
            .unwrap()
            .provenance()
            .unwrap();
        assert_eq!(provenance.source_intervention_id(), &intervention_id);
        assert_eq!(provenance.selected_role_id(), &observer);
    }

    #[test]
    fn intervention_rejects_roles_without_the_required_capability() {
        let definition = definition();
        let mut instance = instance(&definition);

        let error = instance
            .request_intervention_as(
                &definition,
                CeremonyInterventionId::new("not-allowed").unwrap(),
                role_id("observer"),
                CeremonyInterventionKind::Opinion,
                CeremonyInterventionTarget::table(),
                CeremonyInterventionContent::new("What do you think?", Attributes::empty())
                    .unwrap(),
                now(),
            )
            .unwrap_err();

        assert!(matches!(error, DomainError::InvariantViolated { .. }));
    }

    #[test]
    fn rejects_step_execution_outside_current_state() {
        let definition = definition();
        let mut instance = instance(&definition);

        let err = instance
            .start_step(
                &definition,
                &step_id("review_step"),
                lease(
                    "runner-1",
                    "key-1",
                    now(),
                    datetime!(2026-06-06 12:05:00 UTC),
                ),
                now(),
            )
            .unwrap_err();

        assert!(matches!(err, DomainError::InvalidTransition { .. }));
    }

    #[test]
    fn completed_step_unlocks_guarded_transition() {
        let definition = definition();
        let mut instance = instance(&definition);

        instance
            .start_step_as(
                &definition,
                &role_id("facilitator"),
                &step_id("plan"),
                lease(
                    "runner-1",
                    "key-1",
                    now(),
                    datetime!(2026-06-06 12:05:00 UTC),
                ),
                now(),
            )
            .unwrap();
        instance
            .apply_step_result(
                &definition,
                &step_id("plan"),
                StepResult::completed(StepOutput::empty()).unwrap(),
                datetime!(2026-06-06 12:01:00 UTC),
            )
            .unwrap();
        let state = instance
            .apply_transition_as(
                &definition,
                &role_id("facilitator"),
                &trigger("finish"),
                datetime!(2026-06-06 12:02:00 UTC),
            )
            .unwrap();

        assert_eq!(state, state_id("done"));
        assert!(instance.is_completed(&definition));
    }

    #[test]
    fn false_repeat_condition_archives_iteration_and_schedules_the_next() {
        let definition = definition_with_steps(vec![repeating_plan(3)]);
        let mut instance = instance(&definition);

        instance
            .start_step(
                &definition,
                &step_id("plan"),
                lease(
                    "runner-1",
                    "repeat-1",
                    now(),
                    datetime!(2026-06-06 12:05:00 UTC),
                ),
                now(),
            )
            .unwrap();
        instance
            .apply_step_result(
                &definition,
                &step_id("plan"),
                StepResult::completed(readiness_output(false)).unwrap(),
                datetime!(2026-06-06 12:01:00 UTC),
            )
            .unwrap();

        let current = instance.step_record(&step_id("plan")).unwrap();
        assert_eq!(current.status(), StepStatus::Pending);
        assert_eq!(current.iteration().get(), 2);
        assert_eq!(current.attempt(), StepAttempt::FIRST);
        let history = instance.step_record_history(&step_id("plan"));
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].iteration(), StepIteration::FIRST);
        assert_eq!(history[0].output(), &readiness_output(false));
        assert!(instance
            .apply_transition(&definition, &trigger("finish"), now())
            .is_err());

        instance
            .start_step(
                &definition,
                &step_id("plan"),
                lease(
                    "runner-1",
                    "repeat-2",
                    datetime!(2026-06-06 12:02:00 UTC),
                    datetime!(2026-06-06 12:07:00 UTC),
                ),
                datetime!(2026-06-06 12:02:00 UTC),
            )
            .unwrap();
        instance
            .apply_step_result(
                &definition,
                &step_id("plan"),
                StepResult::completed(readiness_output(true)).unwrap(),
                datetime!(2026-06-06 12:03:00 UTC),
            )
            .unwrap();

        let current = instance.step_record(&step_id("plan")).unwrap();
        assert_eq!(current.status(), StepStatus::Completed);
        assert_eq!(current.iteration().get(), 2);
        assert!(!instance.step_repeat_limit_reached(&definition, &step_id("plan")));
        assert_eq!(
            instance
                .apply_transition(&definition, &trigger("finish"), now())
                .unwrap(),
            state_id("done")
        );
    }

    #[test]
    fn repeat_limit_is_terminal_for_the_step_and_blocks_transition() {
        let definition = definition_with_steps(vec![repeating_plan(2)]);
        let mut instance = instance(&definition);

        for iteration in 1..=2 {
            instance
                .start_step(
                    &definition,
                    &step_id("plan"),
                    lease(
                        "runner-1",
                        &format!("limit-{iteration}"),
                        now(),
                        datetime!(2026-06-06 12:05:00 UTC),
                    ),
                    now(),
                )
                .unwrap();
            instance
                .apply_step_result(
                    &definition,
                    &step_id("plan"),
                    StepResult::completed(readiness_output(false)).unwrap(),
                    now(),
                )
                .unwrap();
        }

        assert!(instance.step_repeat_limit_reached(&definition, &step_id("plan")));
        assert_eq!(
            instance
                .step_record(&step_id("plan"))
                .unwrap()
                .iteration()
                .get(),
            2
        );
        assert_eq!(instance.step_record_history(&step_id("plan")).len(), 1);
        assert!(instance
            .apply_transition(&definition, &trigger("finish"), now())
            .is_err());
        assert!(instance
            .start_step(
                &definition,
                &step_id("plan"),
                lease(
                    "runner-1",
                    "limit-3",
                    now(),
                    datetime!(2026-06-06 12:05:00 UTC),
                ),
                now(),
            )
            .is_err());
    }

    #[test]
    fn active_lease_blocks_failover_takeover() {
        let definition = definition();
        let mut instance = instance(&definition);

        instance
            .start_step(
                &definition,
                &step_id("plan"),
                lease(
                    "runner-1",
                    "key-1",
                    now(),
                    datetime!(2026-06-06 12:05:00 UTC),
                ),
                now(),
            )
            .unwrap();
        let err = instance
            .start_step(
                &definition,
                &step_id("plan"),
                lease(
                    "runner-2",
                    "key-2",
                    datetime!(2026-06-06 12:01:00 UTC),
                    datetime!(2026-06-06 12:06:00 UTC),
                ),
                datetime!(2026-06-06 12:01:00 UTC),
            )
            .unwrap_err();

        assert!(matches!(err, DomainError::InvariantViolated { .. }));
        assert_eq!(
            instance
                .step_record(&step_id("plan"))
                .unwrap()
                .lease()
                .unwrap()
                .owner_id()
                .as_str(),
            "runner-1"
        );
    }

    #[test]
    fn expired_lease_allows_failover_takeover_with_next_attempt() {
        let definition = definition();
        let mut instance = instance(&definition);

        instance
            .start_step(
                &definition,
                &step_id("plan"),
                lease(
                    "runner-1",
                    "key-1",
                    now(),
                    datetime!(2026-06-06 12:05:00 UTC),
                ),
                now(),
            )
            .unwrap();
        let attempt = instance
            .start_step(
                &definition,
                &step_id("plan"),
                lease(
                    "runner-2",
                    "key-2",
                    datetime!(2026-06-06 12:06:00 UTC),
                    datetime!(2026-06-06 12:11:00 UTC),
                ),
                datetime!(2026-06-06 12:06:00 UTC),
            )
            .unwrap();

        assert_eq!(attempt, StepAttempt::new(2).unwrap());
        let record = instance.step_record(&step_id("plan")).unwrap();
        assert_eq!(record.attempt(), StepAttempt::new(2).unwrap());
        assert_eq!(record.lease().unwrap().owner_id().as_str(), "runner-2");
    }

    #[test]
    fn approving_a_guard_the_ceremony_never_declared_is_refused() {
        let approval =
            CeremonyGuard::new(guard_name("human_approved"), GuardCondition::HumanApproval);
        let finish = CeremonyTransition::new(
            state_id("drafting"),
            state_id("done"),
            trigger("approve"),
            vec![approval.name().clone()],
        )
        .unwrap();
        let definition = CeremonyDefinition::new(
            crate::value_objects::CeremonyName::new("approval_ceremony").unwrap(),
            CeremonyVersion::v1(),
            None,
            Vec::new(),
            Vec::new(),
            vec![
                CeremonyState::initial(state_id("drafting")),
                CeremonyState::terminal(state_id("done")),
            ],
            vec![finish.clone()],
            Vec::new(),
            vec![approval],
            vec![role(vec![RoleAction::transition(finish.trigger().clone())])],
        )
        .unwrap();
        let mut instance = instance(&definition);

        // This used to succeed and write `not_a_guard: true` into the
        // session context: a caller could put any key at all there,
        // and a typo answered "approved" while leaving a session that
        // would never move.
        assert!(matches!(
            instance.approve_guard(
                &definition,
                &guard_name("not_a_guard"),
                role_id("facilitator"),
                AuditActorKind::Human,
                now()
            ),
            Err(DomainError::NotFound {
                what: "ceremony_guard"
            })
        ));
        assert!(!instance
            .context()
            .is_guard_approved(&guard_name("not_a_guard")));
    }

    #[test]
    fn human_approval_guard_uses_typed_context() {
        let approval =
            CeremonyGuard::new(guard_name("human_approved"), GuardCondition::HumanApproval);
        let finish = CeremonyTransition::new(
            state_id("drafting"),
            state_id("done"),
            trigger("approve"),
            vec![approval.name().clone()],
        )
        .unwrap();
        let definition = CeremonyDefinition::new(
            crate::value_objects::CeremonyName::new("approval_ceremony").unwrap(),
            CeremonyVersion::v1(),
            None,
            Vec::new(),
            Vec::new(),
            vec![
                CeremonyState::initial(state_id("drafting")),
                CeremonyState::terminal(state_id("done")),
            ],
            vec![finish.clone()],
            Vec::new(),
            vec![approval.clone()],
            vec![role(vec![RoleAction::transition(finish.trigger().clone())])],
        )
        .unwrap();
        let mut instance = instance(&definition);

        assert!(matches!(
            instance.apply_transition(&definition, &trigger("approve"), now()),
            Err(DomainError::InvariantViolated { .. })
        ));
        instance
            .approve_guard(
                &definition,
                approval.name(),
                role_id("facilitator"),
                AuditActorKind::Human,
                datetime!(2026-06-06 12:01:00 UTC),
            )
            .unwrap();
        instance
            .apply_transition(
                &definition,
                &trigger("approve"),
                datetime!(2026-06-06 12:02:00 UTC),
            )
            .unwrap();

        assert!(instance.is_completed(&definition));
    }

    #[test]
    fn human_guard_deferral_preserves_uncertainty_without_approving() {
        let approval =
            CeremonyGuard::new(guard_name("human_approved"), GuardCondition::HumanApproval);
        let finish = CeremonyTransition::new(
            state_id("drafting"),
            state_id("done"),
            trigger("approve"),
            vec![approval.name().clone()],
        )
        .unwrap();
        let definition = CeremonyDefinition::new(
            crate::value_objects::CeremonyName::new("deferral_ceremony").unwrap(),
            CeremonyVersion::v1(),
            None,
            Vec::new(),
            Vec::new(),
            vec![
                CeremonyState::initial(state_id("drafting")),
                CeremonyState::terminal(state_id("done")),
            ],
            vec![finish.clone()],
            Vec::new(),
            vec![approval.clone()],
            vec![role(vec![RoleAction::transition(finish.trigger().clone())])],
        )
        .unwrap();
        let mut instance = instance(&definition);

        instance
            .defer_guard(
                &definition,
                approval.name().clone(),
                CeremonyGuardDeferralContent::new(
                    "I do not know.",
                    "I cannot explain how the issue was resolved.",
                    vec!["New evidence explains the resolution.".to_owned()],
                )
                .unwrap(),
                role_id("facilitator"),
                AuditActorKind::Human,
                datetime!(2026-06-06 12:01:00 UTC),
            )
            .unwrap();

        assert!(!instance.context().is_guard_approved(approval.name()));
        assert!(instance
            .apply_transition(&definition, &trigger("approve"), now())
            .is_err());
        let deferral = &instance.guard_deferrals()[0];
        assert_eq!(deferral.guard_name(), approval.name());
        assert_eq!(deferral.content().statement(), "I do not know.");
    }
    /// An approval that names nobody is a receipt for a human decision
    /// nobody can be shown to have taken. This is that made checkable.
    #[test]
    fn approving_a_human_guard_records_the_seat_that_did_it() {
        let approval =
            CeremonyGuard::new(guard_name("human_approved"), GuardCondition::HumanApproval);
        let definition = definition_with_human_guard(&approval);
        let mut instance = instance(&definition);

        instance
            .approve_guard(
                &definition,
                approval.name(),
                role_id("facilitator"),
                AuditActorKind::Human,
                datetime!(2026-06-06 12:01:00 UTC),
            )
            .unwrap();

        let [recorded] = instance.guard_approvals() else {
            panic!(
                "expected one approval, got {:?}",
                instance.guard_approvals()
            );
        };
        assert_eq!(recorded.guard_name(), approval.name());
        assert_eq!(recorded.approved_by(), &role_id("facilitator"));
        assert_eq!(recorded.approved_at(), datetime!(2026-06-06 12:01:00 UTC));
        assert!(instance.context().is_guard_approved(approval.name()));
    }

    /// A seat this session does not have cannot approve anything on it.
    /// Weaker than the capability check the other verbs use, and
    /// deliberately so — but not so weak that any string will do.
    #[test]
    fn a_seat_the_definition_does_not_declare_cannot_approve() {
        let approval =
            CeremonyGuard::new(guard_name("human_approved"), GuardCondition::HumanApproval);
        let definition = definition_with_human_guard(&approval);
        let mut instance = instance(&definition);

        let outcome = instance.approve_guard(
            &definition,
            approval.name(),
            role_id("someone-who-is-not-here"),
            AuditActorKind::Human,
            now(),
        );

        assert!(matches!(
            outcome,
            Err(DomainError::NotFound {
                what: "ceremony_role"
            })
        ));
        assert!(instance.guard_approvals().is_empty());
        assert!(!instance.context().is_guard_approved(approval.name()));
    }
    /// A session with one agenda item and one contribution to it —
    /// the smallest thing that has something to explain.
    fn session_with_a_contribution(
        definition: &CeremonyDefinition,
    ) -> (CeremonyInstance, CeremonyInterventionId) {
        let mut instance = instance(definition);
        let agenda_item = CeremonyInterventionId::new("queue-check").unwrap();
        instance
            .request_intervention_as(
                definition,
                agenda_item.clone(),
                role_id("facilitator"),
                CeremonyInterventionKind::Investigation,
                CeremonyInterventionTarget::roles([role_id("observer")]).unwrap(),
                CeremonyInterventionContent::new("Inspect the queue.", Attributes::empty())
                    .unwrap(),
                now(),
            )
            .unwrap();
        instance
            .respond_to_intervention_as(
                definition,
                &agenda_item,
                role_id("observer"),
                CeremonyInterventionContent::new("Queue depth is stable.", Attributes::empty())
                    .unwrap(),
                now(),
            )
            .unwrap();
        (instance, agenda_item)
    }

    /// The one reason the engine sees on its own, and it records it
    /// without being asked.
    #[test]
    fn a_contribution_is_recorded_as_answering_its_agenda_item() {
        let definition = definition();
        let (instance, agenda_item) = session_with_a_contribution(&definition);

        let [answered] = instance.reasons() else {
            panic!("expected exactly one reason, got {:?}", instance.reasons());
        };
        assert_eq!(answered.kind(), CeremonyReasonKind::Answers);
        assert_eq!(
            answered.from(),
            &CeremonyRecordRef::contribution(agenda_item.clone(), 0)
        );
        assert_eq!(answered.to(), &CeremonyRecordRef::agenda_item(agenda_item));
        assert_eq!(
            answered.asserted_by(),
            None,
            "the engine observed it; naming a seat would be inventing one"
        );
    }

    /// Structure is not a judgement. A seat able to assert it could
    /// rewrite the shape of the session by relabelling it.
    #[test]
    fn a_seat_cannot_assert_what_only_the_engine_observes() {
        let definition = definition();
        let (mut instance, agenda_item) = session_with_a_contribution(&definition);

        let outcome = instance.assert_reason_as(
            &definition,
            role_id("observer"),
            CeremonyRecordRef::contribution(agenda_item.clone(), 0),
            CeremonyRecordRef::agenda_item(agenda_item),
            CeremonyReasonKind::Answers,
            "because I say it does",
            MemoryConfidence::High,
            now(),
        );

        assert!(matches!(
            outcome,
            Err(DomainError::InvariantViolated { .. })
        ));
    }

    /// Testimony about one's own reasoning. Nobody else has access to
    /// it, so nobody else may claim it.
    #[test]
    fn only_whoever_contributed_may_say_why_they_did() {
        let definition = definition();
        let (mut instance, agenda_item) = session_with_a_contribution(&definition);
        let contribution = CeremonyRecordRef::contribution(agenda_item.clone(), 0);
        let item = CeremonyRecordRef::agenda_item(agenda_item);

        let by_someone_else = instance.assert_reason_as(
            &definition,
            role_id("facilitator"),
            contribution.clone(),
            item.clone(),
            CeremonyReasonKind::ChosenBecause,
            "they must have thought the queue mattered",
            MemoryConfidence::Low,
            now(),
        );
        assert!(matches!(
            by_someone_else,
            Err(DomainError::InvariantViolated { .. })
        ));

        instance
            .assert_reason_as(
                &definition,
                role_id("observer"),
                contribution,
                item,
                CeremonyReasonKind::ChosenBecause,
                "the depth graph had been flat for an hour",
                MemoryConfidence::High,
                now(),
            )
            .expect("its author may say why");
        assert_eq!(instance.reasons().len(), 2);
    }

    /// A claim about the world, not about a mind. Anyone may make one
    /// and everyone may weigh it.
    #[test]
    fn any_seat_may_claim_that_one_thing_came_from_another() {
        let definition = definition();
        let (mut instance, agenda_item) = session_with_a_contribution(&definition);

        instance
            .assert_reason_as(
                &definition,
                role_id("facilitator"),
                CeremonyRecordRef::agenda_item(agenda_item.clone()),
                CeremonyRecordRef::contribution(agenda_item, 0),
                CeremonyReasonKind::FollowsFrom,
                "the item stayed open because the answer raised a new question",
                MemoryConfidence::Medium,
                now(),
            )
            .expect("a claim about the world is open to any seat");

        let asserted = instance.reasons().last().unwrap();
        assert_eq!(asserted.confidence(), MemoryConfidence::Medium);
        assert_eq!(asserted.asserted_by(), Some(&role_id("facilitator")));
    }

    /// A session knows everything it has done, so a reason may not
    /// cite something it never produced.
    #[test]
    fn a_reason_cannot_cite_something_that_never_happened() {
        let definition = definition();
        let (mut instance, agenda_item) = session_with_a_contribution(&definition);

        let outcome = instance.assert_reason_as(
            &definition,
            role_id("observer"),
            CeremonyRecordRef::contribution(agenda_item.clone(), 7),
            CeremonyRecordRef::agenda_item(agenda_item),
            CeremonyReasonKind::FollowsFrom,
            "a contribution nobody made",
            MemoryConfidence::Low,
            now(),
        );

        assert!(matches!(
            outcome,
            Err(DomainError::NotFound {
                what: "ceremony_record"
            })
        ));
    }

    /// A move is recorded with the seat that fired it, so "the session
    /// resolved because…" has something to point at.
    #[test]
    fn a_move_is_recorded_with_whoever_made_it() {
        let approval =
            CeremonyGuard::new(guard_name("human_approved"), GuardCondition::HumanApproval);
        let definition = definition_with_human_guard(&approval);
        let mut instance = instance(&definition);
        instance
            .approve_guard(
                &definition,
                approval.name(),
                role_id("facilitator"),
                AuditActorKind::Human,
                now(),
            )
            .unwrap();

        instance
            .apply_transition_as(
                &definition,
                &role_id("facilitator"),
                &trigger("approve"),
                datetime!(2026-06-06 12:05:00 UTC),
            )
            .unwrap();

        let [moved] = instance.transitions() else {
            panic!("expected one move, got {:?}", instance.transitions());
        };
        assert_eq!(moved.trigger(), &trigger("approve"));
        assert_eq!(moved.from_state(), &state_id("drafting"));
        assert_eq!(moved.to_state(), &state_id("done"));
        assert_eq!(moved.applied_by(), Some(&role_id("facilitator")));
    }

    /// And without one when the engine took the move itself. An
    /// absence, not a gap — and it is what stops testimony being
    /// claimed about something nobody can testify to.
    #[test]
    fn a_move_the_engine_took_names_nobody() {
        let approval =
            CeremonyGuard::new(guard_name("human_approved"), GuardCondition::HumanApproval);
        let definition = definition_with_human_guard(&approval);
        let mut instance = instance(&definition);
        instance
            .approve_guard(
                &definition,
                approval.name(),
                role_id("facilitator"),
                AuditActorKind::Human,
                now(),
            )
            .unwrap();

        instance
            .apply_transition(&definition, &trigger("approve"), now())
            .unwrap();

        assert_eq!(instance.transitions()[0].applied_by(), None);
    }
    /// What kind of party filled the seat is recorded as declared and
    /// never inferred.
    ///
    /// The engine knows this guard demands a human. That says one was
    /// required, not that one turned up — and a receipt that read
    /// compliance off its own requirement would assert exactly what
    /// nobody can demonstrate. So an agent approving a human-approval
    /// guard is recorded as an agent, and whether that is acceptable
    /// is a question for whoever reads it.
    #[test]
    fn an_approval_records_the_kind_it_was_told_not_the_one_the_guard_wanted() {
        let approval =
            CeremonyGuard::new(guard_name("human_approved"), GuardCondition::HumanApproval);
        let definition = definition_with_human_guard(&approval);
        let mut instance = instance(&definition);

        instance
            .approve_guard(
                &definition,
                approval.name(),
                role_id("facilitator"),
                AuditActorKind::Agent,
                now(),
            )
            .unwrap();

        let [recorded] = instance.guard_approvals() else {
            panic!("expected one approval");
        };
        assert_eq!(
            recorded.approved_by_kind(),
            AuditActorKind::Agent,
            "the guard asked for a human and an agent answered; saying otherwise \
             would be the engine vouching for something it cannot see"
        );
        assert!(instance.context().is_guard_approved(approval.name()));
    }
}
