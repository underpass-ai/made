//! Conformance suite for [`AgenticSystemRepositoryPort`].
//!
//! The property under test is that no edit is ever lost. It is easy to
//! fail by accident: a store whose `save` writes whatever it is given
//! looks correct until two people edit one design, and then the second
//! write silently erases the first — which is exactly the situation a
//! design document is most often in.

use crate::error::DomainError;
use crate::ports::{
    AgenticSystemPage, AgenticSystemQuery, AgenticSystemRepositoryPort, AgenticSystemSaveOutcome,
};
use crate::value_objects::{AgenticSystemLifecycle, AgenticSystemPageLimit, AgenticSystemRevision};

use super::agentic_system_fixtures::{call, design, failure, system_id};
use super::ConformanceFailure;

/// Every property an [`AgenticSystemRepositoryPort`] implementation
/// must satisfy.
#[derive(Debug)]
pub struct AgenticSystemRepositoryConformance;

impl AgenticSystemRepositoryConformance {
    pub async fn run(
        repository: &dyn AgenticSystemRepositoryPort,
    ) -> Result<Vec<&'static str>, ConformanceFailure> {
        let mut passed = Vec::new();
        Self::an_unsaved_design_is_absent(repository).await?;
        passed.push("an_unsaved_design_is_absent");
        Self::creation_is_the_only_save_without_an_expected_revision(repository).await?;
        passed.push("creation_is_the_only_save_without_an_expected_revision");
        Self::a_concurrent_edit_is_refused_rather_than_overwritten(repository).await?;
        passed.push("a_concurrent_edit_is_refused_rather_than_overwritten");
        Self::an_earlier_revision_stays_readable(repository).await?;
        passed.push("an_earlier_revision_stays_readable");
        Self::listing_is_bounded_and_resumable(repository).await?;
        passed.push("listing_is_bounded_and_resumable");
        Ok(passed)
    }

    async fn an_unsaved_design_is_absent(
        repository: &dyn AgenticSystemRepositoryPort,
    ) -> Result<(), ConformanceFailure> {
        const PROPERTY: &str = "an_unsaved_design_is_absent";
        let id = system_id(PROPERTY)?;

        if call(PROPERTY, repository.get(&id, None).await)?.is_some() {
            return Err(failure(PROPERTY, "a design that was never saved came back"));
        }
        Ok(())
    }

    async fn creation_is_the_only_save_without_an_expected_revision(
        repository: &dyn AgenticSystemRepositoryPort,
    ) -> Result<(), ConformanceFailure> {
        const PROPERTY: &str = "creation_is_the_only_save_without_an_expected_revision";
        let id = system_id(PROPERTY)?;
        let first = design(PROPERTY, &id, "the first intent")?;

        let created = call(PROPERTY, repository.save(first.clone(), None).await)?;
        if created != AgenticSystemSaveOutcome::saved(AgenticSystemRevision::INITIAL) {
            return Err(failure(
                PROPERTY,
                format!("creating did not answer with the first revision: {created:?}"),
            ));
        }

        // A second create is somebody who believes this design does
        // not exist. Accepting it would replace a design they have
        // never read.
        let again = call(PROPERTY, repository.save(first, None).await)?;
        if !again.is_conflict() {
            return Err(failure(
                PROPERTY,
                "creating a design that already exists was accepted",
            ));
        }
        Ok(())
    }

    async fn a_concurrent_edit_is_refused_rather_than_overwritten(
        repository: &dyn AgenticSystemRepositoryPort,
    ) -> Result<(), ConformanceFailure> {
        const PROPERTY: &str = "a_concurrent_edit_is_refused_rather_than_overwritten";
        let id = system_id(PROPERTY)?;
        let read = design(PROPERTY, &id, "the intent both editors read")?;
        call(PROPERTY, repository.save(read.clone(), None).await)?;

        let mine = design(PROPERTY, &id, "what I changed it to")?
            .edited(crate::conformance::agentic_system_fixtures::origin());
        let theirs = design(PROPERTY, &id, "what they changed it to")?
            .edited(crate::conformance::agentic_system_fixtures::origin());

        let won = call(
            PROPERTY,
            repository
                .save(mine, Some(AgenticSystemRevision::INITIAL))
                .await,
        )?;
        if won.revision().map(AgenticSystemRevision::get) != Some(2) {
            return Err(failure(
                PROPERTY,
                format!("the first editor was not given the next revision: {won:?}"),
            ));
        }

        let lost = call(
            PROPERTY,
            repository
                .save(theirs, Some(AgenticSystemRevision::INITIAL))
                .await,
        )?;
        let AgenticSystemSaveOutcome::RevisionConflict { current } = lost else {
            return Err(failure(
                PROPERTY,
                "the second editor's stale save was accepted",
            ));
        };
        if current.get() != 2 {
            return Err(failure(
                PROPERTY,
                format!("the conflict named revision {current} instead of the head"),
            ));
        }

        let head = call(PROPERTY, repository.get(&id, None).await)?
            .ok_or_else(|| failure(PROPERTY, "the head disappeared"))?;
        if head.purpose().as_str() != "what I changed it to" {
            return Err(failure(
                PROPERTY,
                "the refused edit reached the store anyway",
            ));
        }
        Ok(())
    }

    async fn an_earlier_revision_stays_readable(
        repository: &dyn AgenticSystemRepositoryPort,
    ) -> Result<(), ConformanceFailure> {
        const PROPERTY: &str = "an_earlier_revision_stays_readable";
        let id = system_id(PROPERTY)?;
        let first = design(PROPERTY, &id, "as it was first written")?;
        call(PROPERTY, repository.save(first, None).await)?;
        let second = design(PROPERTY, &id, "as it was rewritten")?
            .edited(crate::conformance::agentic_system_fixtures::origin());
        call(
            PROPERTY,
            repository
                .save(second, Some(AgenticSystemRevision::INITIAL))
                .await,
        )?;

        let earlier = call(
            PROPERTY,
            repository
                .get(&id, Some(AgenticSystemRevision::INITIAL))
                .await,
        )?
        .ok_or_else(|| failure(PROPERTY, "the first revision was not kept"))?;
        if earlier.purpose().as_str() != "as it was first written" {
            return Err(failure(
                PROPERTY,
                "reading an earlier revision answered with the head",
            ));
        }
        Ok(())
    }

    async fn listing_is_bounded_and_resumable(
        repository: &dyn AgenticSystemRepositoryPort,
    ) -> Result<(), ConformanceFailure> {
        const PROPERTY: &str = "listing_is_bounded_and_resumable";
        for suffix in ["a", "b"] {
            let id = crate::value_objects::AgenticSystemId::new(format!(
                "{}-{suffix}",
                PROPERTY.replace('_', "-")
            ))
            .map_err(|error| failure(PROPERTY, error.to_string()))?;
            let design = design(PROPERTY, &id, "listed")?;
            call(PROPERTY, repository.save(design, None).await)?;
        }
        let limit =
            AgenticSystemPageLimit::new(1).map_err(|error| failure(PROPERTY, error.to_string()))?;

        let first = page(
            PROPERTY,
            repository,
            &AgenticSystemQuery::new(None, limit, None),
        )
        .await?;
        if first.systems().len() != 1 {
            return Err(failure(
                PROPERTY,
                format!("a limit of one returned {} designs", first.systems().len()),
            ));
        }
        let cursor = first
            .next_cursor()
            .cloned()
            .ok_or_else(|| failure(PROPERTY, "a full page offered no cursor"))?;
        let second = page(
            PROPERTY,
            repository,
            &AgenticSystemQuery::new(None, limit, Some(cursor.clone())),
        )
        .await?;
        if second.systems().iter().any(|system| system.id() == &cursor) {
            return Err(failure(
                PROPERTY,
                "the second page repeated the design the cursor named",
            ));
        }

        let drafts_only = page(
            PROPERTY,
            repository,
            &AgenticSystemQuery::new(Some(AgenticSystemLifecycle::Published), limit, None),
        )
        .await?;
        if drafts_only
            .systems()
            .iter()
            .any(|system| system.lifecycle() != AgenticSystemLifecycle::Published)
        {
            return Err(failure(PROPERTY, "a lifecycle filter was not applied"));
        }
        Ok(())
    }
}

async fn page(
    property: &'static str,
    repository: &dyn AgenticSystemRepositoryPort,
    query: &AgenticSystemQuery,
) -> Result<AgenticSystemPage, ConformanceFailure> {
    let listed: Result<AgenticSystemPage, DomainError> = repository.list(query).await;
    call(property, listed)
}
