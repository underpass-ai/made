//! Conformance suite for [`AgenticSystemExecutionStorePort`].
//!
//! Two properties, and both are about a host asking twice. Opening a
//! run with an identifier that already names one must hand back the
//! run that exists rather than starting a second, and writing a run
//! back against a version somebody else has already moved past must be
//! refused rather than silently reverting their progress.

use crate::entities::AgenticSystemExecution;
use crate::ports::AgenticSystemExecutionStorePort;
use crate::value_objects::{CeremonyExecutionLink, CeremonyId, LoopRound, SystemCeremonyId};

use super::agentic_system_fixtures::{call, failure, origin, pin, run, sealed, system_id};
use super::ConformanceFailure;

/// Every property an [`AgenticSystemExecutionStorePort`]
/// implementation must satisfy.
#[derive(Debug)]
pub struct AgenticSystemExecutionStoreConformance;

impl AgenticSystemExecutionStoreConformance {
    pub async fn run(
        store: &dyn AgenticSystemExecutionStorePort,
    ) -> Result<Vec<&'static str>, ConformanceFailure> {
        let mut passed = Vec::new();
        Self::an_unopened_run_is_absent(store).await?;
        passed.push("an_unopened_run_is_absent");
        Self::opening_the_same_run_twice_returns_the_first(store).await?;
        passed.push("opening_the_same_run_twice_returns_the_first");
        Self::a_stale_update_is_refused_with_what_is_stored(store).await?;
        passed.push("a_stale_update_is_refused_with_what_is_stored");
        Self::runs_are_findable_by_the_design_they_run(store).await?;
        passed.push("runs_are_findable_by_the_design_they_run");
        Ok(passed)
    }

    async fn an_unopened_run_is_absent(
        store: &dyn AgenticSystemExecutionStorePort,
    ) -> Result<(), ConformanceFailure> {
        const PROPERTY: &str = "an_unopened_run_is_absent";
        let id = super::agentic_system_fixtures::execution_id(PROPERTY, "absent")?;

        if call(PROPERTY, store.get(&id).await)?.is_some() {
            return Err(failure(PROPERTY, "a run that was never opened came back"));
        }
        Ok(())
    }

    async fn opening_the_same_run_twice_returns_the_first(
        store: &dyn AgenticSystemExecutionStorePort,
    ) -> Result<(), ConformanceFailure> {
        const PROPERTY: &str = "opening_the_same_run_twice_returns_the_first";
        let design = sealed(PROPERTY, &system_id(PROPERTY)?, "retried")?;
        let opened = run(PROPERTY, "once", &design)?;

        let first = call(PROPERTY, store.create(opened.clone()).await)?;
        if !first.is_new() {
            return Err(failure(PROPERTY, "a first opening was reported as a retry"));
        }
        let again = call(PROPERTY, store.create(opened.clone()).await)?;
        if again.is_new() {
            return Err(failure(
                PROPERTY,
                "opening the same run twice opened a second one",
            ));
        }
        if again.execution().id() != opened.id() {
            return Err(failure(PROPERTY, "the retry answered with a different run"));
        }
        Ok(())
    }

    async fn a_stale_update_is_refused_with_what_is_stored(
        store: &dyn AgenticSystemExecutionStorePort,
    ) -> Result<(), ConformanceFailure> {
        const PROPERTY: &str = "a_stale_update_is_refused_with_what_is_stored";
        let design = sealed(PROPERTY, &system_id(PROPERTY)?, "advanced twice")?;
        let opened = run(PROPERTY, "raced", &design)?;
        call(PROPERTY, store.create(opened.clone()).await)?;

        let mine = advanced(PROPERTY, &opened, "c-mine", 1)?;
        let theirs = advanced(PROPERTY, &opened, "c-theirs", 2)?;

        let won = call(PROPERTY, store.update(mine, opened.updated_at()).await)?;
        if won.is_conflict() {
            return Err(failure(PROPERTY, "the first advance was refused"));
        }
        let lost = call(PROPERTY, store.update(theirs, opened.updated_at()).await)?;
        if !lost.is_conflict() {
            return Err(failure(
                PROPERTY,
                "a stale advance overwrote one that had already landed",
            ));
        }
        let ceremony = SystemCeremonyId::new("delivery")
            .map_err(|error| failure(PROPERTY, error.to_string()))?;
        let instance = lost
            .execution()
            .link(&ceremony)
            .and_then(CeremonyExecutionLink::instance_id);
        if instance.map(CeremonyId::as_str) != Some("c-mine") {
            return Err(failure(
                PROPERTY,
                "the conflict did not carry the run as it actually stands",
            ));
        }
        Ok(())
    }

    async fn runs_are_findable_by_the_design_they_run(
        store: &dyn AgenticSystemExecutionStorePort,
    ) -> Result<(), ConformanceFailure> {
        const PROPERTY: &str = "runs_are_findable_by_the_design_they_run";
        let id = system_id(PROPERTY)?;
        let design = sealed(PROPERTY, &id, "listed by system")?;
        call(PROPERTY, store.create(run(PROPERTY, "one", &design)?).await)?;
        call(PROPERTY, store.create(run(PROPERTY, "two", &design)?).await)?;

        let found = call(PROPERTY, store.list_by_system(&id).await)?;
        if found.len() != 2 {
            return Err(failure(
                PROPERTY,
                format!("two runs of one design listed as {}", found.len()),
            ));
        }
        Ok(())
    }
}

/// The same run with its one composition started, a second later so
/// the stored version differs from the one the caller read.
fn advanced(
    property: &'static str,
    execution: &AgenticSystemExecution,
    instance: &str,
    seconds: i64,
) -> Result<AgenticSystemExecution, ConformanceFailure> {
    let ceremony =
        SystemCeremonyId::new("delivery").map_err(|error| failure(property, error.to_string()))?;
    let link = CeremonyExecutionLink::pending(
        pin(0x0a).map_err(|error| failure(property, error.to_string()))?,
    )
    .started(
        CeremonyId::new(instance).map_err(|error| failure(property, error.to_string()))?,
        LoopRound::ZERO.next(),
    );
    execution
        .with_link(&ceremony, link, origin() + time::Duration::seconds(seconds))
        .map_err(|error| failure(property, error.to_string()))
}
