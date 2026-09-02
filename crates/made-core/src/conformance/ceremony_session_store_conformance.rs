//! Conformance suite for the two ports a session store must serve at
//! once: [`CeremonyInstanceRepositoryPort`] and
//! [`CeremonyUnitOfWorkPort`].
//!
//! Each port has its own suite and an adapter can pass both while being
//! unusable, because the properties that matter here are not about
//! either port — they are about the two of them describing the same
//! storage.
//!
//! Two adapters over two storages satisfy every property either suite
//! can state. A session committed through the unit of work simply never
//! appears to the reader, every call returns `Ok`, and nothing in the
//! logs says a thing. The suite exists because that failure is
//! invisible from inside either contract.
//!
//! # What this suite cannot check
//!
//! **That the storage is the same one under load.** These properties
//! are sequential: commit, then read. An adapter that shares storage
//! but publishes writes late — a replica read, a cache with its own
//! expiry — passes here and still hands a caller a session that has
//! already moved on. Whether a read reflects a commit that has already
//! returned is a property of the store, and the host proves it against
//! its own.

use time::OffsetDateTime;

use crate::entities::CeremonyInstance;
use crate::error::DomainError;
use crate::ports::{CeremonyInstanceRepositoryPort, CeremonyUnitOfWorkPort};
use crate::value_objects::{
    Attributes, CeremonyContext, CeremonyId, CeremonyName, CeremonyRole, CeremonyState,
    CeremonyStep, CeremonyTransition, CeremonyVersion, ExpectedRevision, IdempotencyKey,
    LeaseOwnerId, RepeatUntilCondition, RetryPolicy, RoleAction, RoleId, StateId,
    StepHandlerConfig, StepHandlerKind, StepId, StepIteration, StepLease, StepOutput,
    StepOutputField, StepRepeatPolicy, StepResult, TransitionTrigger,
};
use serde_json::json;

use super::conformance_fixtures::{commit_with, definition};
use super::ConformanceFailure;

/// Every property that must hold when one store serves both the
/// reading of sessions and the committing of them.
#[derive(Debug)]
pub struct CeremonySessionStoreConformance;

impl CeremonySessionStoreConformance {
    /// Run the whole suite. The two arguments are meant to be the same
    /// object; passing two is how the suite gets to find out whether
    /// they are.
    pub async fn run(
        instances: &dyn CeremonyInstanceRepositoryPort,
        unit_of_work: &dyn CeremonyUnitOfWorkPort,
    ) -> Result<Vec<&'static str>, ConformanceFailure> {
        let mut passed = Vec::new();
        Self::an_unstored_session_is_not_found(instances).await?;
        passed.push("an_unstored_session_is_not_found");
        Self::a_committed_session_can_be_read_back(instances, unit_of_work).await?;
        passed.push("a_committed_session_can_be_read_back");
        Self::a_plain_save_is_visible_to_the_next_commit(instances, unit_of_work).await?;
        passed.push("a_plain_save_is_visible_to_the_next_commit");
        Self::semantic_iteration_history_survives_storage(instances).await?;
        passed.push("semantic_iteration_history_survives_storage");
        Ok(passed)
    }

    /// A store that reports sessions it was never given cannot be
    /// trusted to report the ones it was.
    async fn an_unstored_session_is_not_found(
        instances: &dyn CeremonyInstanceRepositoryPort,
    ) -> Result<(), ConformanceFailure> {
        const PROPERTY: &str = "an_unstored_session_is_not_found";
        let ceremony = ceremony_id(PROPERTY, "unstored")?;

        if call(PROPERTY, instances.exists(&ceremony).await)? {
            return Err(failure(
                PROPERTY,
                "a session that was never stored is reported as existing",
            ));
        }
        match instances.get(&ceremony).await {
            Err(DomainError::NotFound { .. }) => Ok(()),
            Ok(_) => Err(failure(
                PROPERTY,
                "reading a session that was never stored returned one",
            )),
            Err(error) => Err(failure(
                PROPERTY,
                format!("reading an unstored session failed with {error}, expected not-found"),
            )),
        }
    }

    /// The property the whole arrangement rests on.
    ///
    /// A commit that returns without the session becoming readable is
    /// the two-storages bug, and it looks like success from every
    /// vantage point except this one.
    async fn a_committed_session_can_be_read_back(
        instances: &dyn CeremonyInstanceRepositoryPort,
        unit_of_work: &dyn CeremonyUnitOfWorkPort,
    ) -> Result<(), ConformanceFailure> {
        const PROPERTY: &str = "a_committed_session_can_be_read_back";
        let ceremony = ceremony_id(PROPERTY, "committed")?;

        let outcome = call(
            PROPERTY,
            unit_of_work
                .commit(commit(PROPERTY, &ceremony, ExpectedRevision::New)?)
                .await,
        )?;
        if outcome.committed_revision().is_none() {
            return Err(failure(
                PROPERTY,
                "a first commit against an unstored session conflicted",
            ));
        }

        if !call(PROPERTY, instances.exists(&ceremony).await)? {
            return Err(failure(
                PROPERTY,
                "a committed session is not reported as existing: the two ports are not over one \
                 storage",
            ));
        }
        let read_back = call(PROPERTY, instances.get(&ceremony).await)?;
        if read_back.id() != &ceremony {
            return Err(failure(
                PROPERTY,
                "reading a committed session returned a different one",
            ));
        }
        if !call(PROPERTY, instances.list().await)?
            .iter()
            .any(|instance| instance.id() == &ceremony)
        {
            return Err(failure(
                PROPERTY,
                "a committed session is missing from the listing",
            ));
        }
        Ok(())
    }

    /// A save outside the unit of work must still move the revision.
    ///
    /// Both ports can write, and only one of them checks. If the
    /// unchecked path leaves the revision where it was, a commit
    /// holding an expectation from before that save is accepted and
    /// overwrites it — the weaker path silently defeating the stronger
    /// one, with the conflict machinery working perfectly throughout.
    async fn a_plain_save_is_visible_to_the_next_commit(
        instances: &dyn CeremonyInstanceRepositoryPort,
        unit_of_work: &dyn CeremonyUnitOfWorkPort,
    ) -> Result<(), ConformanceFailure> {
        const PROPERTY: &str = "a_plain_save_is_visible_to_the_next_commit";
        let ceremony = ceremony_id(PROPERTY, "saved")?;

        call(
            PROPERTY,
            unit_of_work
                .commit(commit(PROPERTY, &ceremony, ExpectedRevision::New)?)
                .await,
        )?;
        let Some(before) = call(PROPERTY, unit_of_work.revision(&ceremony).await)? else {
            return Err(failure(PROPERTY, "a committed session reports no revision"));
        };

        call(
            PROPERTY,
            instances.save(&session(PROPERTY, &ceremony)?).await,
        )?;

        let outcome = call(
            PROPERTY,
            unit_of_work
                .commit(commit(
                    PROPERTY,
                    &ceremony,
                    ExpectedRevision::Exactly(before),
                )?)
                .await,
        )?;
        if outcome.committed_revision().is_some() {
            return Err(failure(
                PROPERTY,
                "a commit expecting the revision from before a plain save was accepted: the save \
                 can be overwritten without anyone noticing",
            ));
        }
        Ok(())
    }

    /// A successful semantic iteration is history, not scratch space for the
    /// next record. Stores must preserve both after serialization.
    async fn semantic_iteration_history_survives_storage(
        instances: &dyn CeremonyInstanceRepositoryPort,
    ) -> Result<(), ConformanceFailure> {
        const PROPERTY: &str = "semantic_iteration_history_survives_storage";
        let ceremony = ceremony_id(PROPERTY, "repeat")?;
        let (definition, mut instance, step_id) = repeating_session(PROPERTY, ceremony)?;
        let now = OffsetDateTime::UNIX_EPOCH;
        let lease = call(
            PROPERTY,
            LeaseOwnerId::new("conformance-runner").and_then(|owner| {
                IdempotencyKey::new("semantic-iteration-1").and_then(|key| {
                    StepLease::new(owner, key, now, now + time::Duration::seconds(60))
                })
            }),
        )?;
        call(
            PROPERTY,
            instance.start_step(&definition, &step_id, lease, now),
        )?;
        let result = call(
            PROPERTY,
            Attributes::new([("ready".to_owned(), json!(false))].into_iter().collect())
                .map(StepOutput::new)
                .and_then(StepResult::completed),
        )?;
        call(
            PROPERTY,
            instance.apply_step_result(&definition, &step_id, result, now),
        )?;

        call(PROPERTY, instances.save(&instance).await)?;
        let restored = call(PROPERTY, instances.get(instance.id()).await)?;

        if restored != instance
            || restored.step_record_history(&step_id).len() != 1
            || restored
                .step_record(&step_id)
                .is_none_or(|record| record.iteration().get() != 2)
        {
            return Err(failure(
                PROPERTY,
                "the current semantic iteration or its preceding record changed in storage",
            ));
        }
        Ok(())
    }
}

fn repeating_session(
    property: &'static str,
    ceremony_id: CeremonyId,
) -> Result<
    (
        crate::entities::CeremonyDefinition,
        CeremonyInstance,
        StepId,
    ),
    ConformanceFailure,
> {
    let build = || -> Result<_, DomainError> {
        let open = StateId::new("OPEN")?;
        let done = StateId::new("DONE")?;
        let step_id = StepId::new("observe")?;
        let trigger = TransitionTrigger::new("finish")?;
        let step = CeremonyStep::new(
            step_id.clone(),
            open.clone(),
            StepHandlerKind::new("noop")?,
            StepHandlerConfig::empty(),
            RetryPolicy::single_attempt(),
            None,
        )
        .with_repeat_policy(StepRepeatPolicy::new(
            RepeatUntilCondition::output_field_equals(StepOutputField::new("ready")?, json!(true)),
            StepIteration::new(3)?,
        ));
        let transition =
            CeremonyTransition::new(open.clone(), done.clone(), trigger.clone(), Vec::new())?;
        let definition = crate::entities::CeremonyDefinition::new(
            CeremonyName::new("repeat_conformance")?,
            CeremonyVersion::v1(),
            None,
            Vec::new(),
            Vec::new(),
            vec![CeremonyState::initial(open), CeremonyState::terminal(done)],
            vec![transition],
            vec![step],
            Vec::new(),
            vec![CeremonyRole::new(
                RoleId::new("runner")?,
                vec![
                    RoleAction::step(step_id.clone()),
                    RoleAction::transition(trigger),
                ],
            )?],
        )?;
        let instance = CeremonyInstance::start(
            ceremony_id,
            &definition,
            CeremonyContext::empty(),
            OffsetDateTime::UNIX_EPOCH,
        );
        Ok((definition, instance, step_id))
    };
    build().map_err(|error| failure(property, format!("repeat fixture rejected: {error}")))
}

fn call<T>(
    property: &'static str,
    result: Result<T, DomainError>,
) -> Result<T, ConformanceFailure> {
    result.map_err(|error| failure(property, format!("call failed: {error}")))
}

fn failure(property: &'static str, detail: impl Into<String>) -> ConformanceFailure {
    ConformanceFailure::new(property, detail)
}

fn ceremony_id(property: &'static str, suffix: &str) -> Result<CeremonyId, ConformanceFailure> {
    CeremonyId::new(format!("session-store-{property}-{suffix}"))
        .map_err(|error| failure(property, format!("fixture id rejected: {error}")))
}

fn commit(
    property: &'static str,
    ceremony_id: &CeremonyId,
    expected: ExpectedRevision,
) -> Result<crate::entities::CeremonyCommit, ConformanceFailure> {
    // Facts are named after the expectation they were committed under,
    // so two commits in one property cannot collide on an event id.
    let event = match expected {
        ExpectedRevision::New => "new",
        ExpectedRevision::Exactly(_) => "next",
    };
    commit_with(ceremony_id, expected, &format!("{property}-{event}"), &[])
        .map_err(|error| failure(property, format!("fixture commit rejected: {error}")))
}

fn session(
    property: &'static str,
    ceremony_id: &CeremonyId,
) -> Result<CeremonyInstance, ConformanceFailure> {
    let definition =
        definition().map_err(|error| failure(property, format!("fixture rejected: {error}")))?;
    Ok(CeremonyInstance::start(
        ceremony_id.clone(),
        &definition,
        CeremonyContext::empty(),
        OffsetDateTime::UNIX_EPOCH,
    ))
}
