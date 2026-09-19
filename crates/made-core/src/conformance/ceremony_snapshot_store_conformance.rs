//! Conformance suite for [`CeremonySnapshotStorePort`].
//!
//! A snapshot store is a cache with two promises: the latest snapshot
//! is the one at the highest version, and forgetting a stream leaves
//! every other stream alone. That the fold without snapshots reaches
//! the same state is a property of the aggregate and is proven where
//! the fold lives, not here.

use time::OffsetDateTime;

use crate::entities::ceremony_events::ExecutionReceiptLinked;
use crate::entities::{CeremonyEvent, CeremonyInstance};
use crate::error::DomainError;
use crate::ports::{CeremonySnapshot, CeremonySnapshotStorePort};
use crate::value_objects::{
    CeremonyContext, CeremonyId, ExecutionOperationId, ExecutionReceiptId, ExecutionReceiptLink,
    ExecutionReceiptLinkKind, StateIteration, StateVisit, StepClaimFence, StepId, StepIteration,
    StreamVersion,
};

use super::conformance_fixtures::definition;
use super::ConformanceFailure;

/// Every property a [`CeremonySnapshotStorePort`] implementation must
/// satisfy.
#[derive(Debug)]
pub struct CeremonySnapshotStoreConformance;

impl CeremonySnapshotStoreConformance {
    /// Run the whole suite against one store. Each property uses its
    /// own streams, so a store other properties wrote to is fine.
    pub async fn run(
        store: &dyn CeremonySnapshotStorePort,
    ) -> Result<Vec<&'static str>, ConformanceFailure> {
        let mut passed = Vec::new();
        Self::an_unknown_stream_has_no_snapshot(store).await?;
        passed.push("an_unknown_stream_has_no_snapshot");
        Self::a_saved_snapshot_is_the_latest(store).await?;
        passed.push("a_saved_snapshot_is_the_latest");
        Self::a_higher_version_becomes_latest_and_a_lower_one_does_not(store).await?;
        passed.push("a_higher_version_becomes_latest_and_a_lower_one_does_not");
        Self::saving_the_same_version_twice_is_idempotent(store).await?;
        passed.push("saving_the_same_version_twice_is_idempotent");
        Self::receipt_adoptions_survive_snapshot_reopen(store).await?;
        passed.push("receipt_adoptions_survive_snapshot_reopen");
        Self::forgetting_a_stream_drops_its_snapshots_and_no_others(store).await?;
        passed.push("forgetting_a_stream_drops_its_snapshots_and_no_others");
        Ok(passed)
    }

    async fn an_unknown_stream_has_no_snapshot(
        store: &dyn CeremonySnapshotStorePort,
    ) -> Result<(), ConformanceFailure> {
        const PROPERTY: &str = "an_unknown_stream_has_no_snapshot";
        let stream = ceremony_id(PROPERTY, "unknown")?;

        if call(PROPERTY, store.latest(&stream).await)?.is_some() {
            return Err(failure(
                PROPERTY,
                "a stream nothing was saved for has a snapshot",
            ));
        }
        Ok(())
    }

    async fn a_saved_snapshot_is_the_latest(
        store: &dyn CeremonySnapshotStorePort,
    ) -> Result<(), ConformanceFailure> {
        const PROPERTY: &str = "a_saved_snapshot_is_the_latest";
        let stream = ceremony_id(PROPERTY, "saved")?;

        let snapshot = snapshot(PROPERTY, &stream, 3)?;
        call(PROPERTY, store.save(snapshot.clone()).await)?;
        expect_latest(store, PROPERTY, &stream, Some(&snapshot)).await
    }

    async fn a_higher_version_becomes_latest_and_a_lower_one_does_not(
        store: &dyn CeremonySnapshotStorePort,
    ) -> Result<(), ConformanceFailure> {
        const PROPERTY: &str = "a_higher_version_becomes_latest_and_a_lower_one_does_not";
        let stream = ceremony_id(PROPERTY, "ordered")?;

        call(PROPERTY, store.save(snapshot(PROPERTY, &stream, 2)?).await)?;
        let higher = snapshot(PROPERTY, &stream, 5)?;
        call(PROPERTY, store.save(higher.clone()).await)?;
        expect_latest(store, PROPERTY, &stream, Some(&higher)).await?;

        call(PROPERTY, store.save(snapshot(PROPERTY, &stream, 4)?).await)?;
        expect_latest(store, PROPERTY, &stream, Some(&higher)).await
    }

    async fn saving_the_same_version_twice_is_idempotent(
        store: &dyn CeremonySnapshotStorePort,
    ) -> Result<(), ConformanceFailure> {
        const PROPERTY: &str = "saving_the_same_version_twice_is_idempotent";
        let stream = ceremony_id(PROPERTY, "twice")?;

        let snapshot = snapshot(PROPERTY, &stream, 1)?;
        call(PROPERTY, store.save(snapshot.clone()).await)?;
        call(PROPERTY, store.save(snapshot.clone()).await)?;
        expect_latest(store, PROPERTY, &stream, Some(&snapshot)).await
    }

    async fn forgetting_a_stream_drops_its_snapshots_and_no_others(
        store: &dyn CeremonySnapshotStorePort,
    ) -> Result<(), ConformanceFailure> {
        const PROPERTY: &str = "forgetting_a_stream_drops_its_snapshots_and_no_others";
        let forgotten = ceremony_id(PROPERTY, "forgotten")?;
        let kept = ceremony_id(PROPERTY, "kept")?;

        for version in [1, 2] {
            call(
                PROPERTY,
                store.save(snapshot(PROPERTY, &forgotten, version)?).await,
            )?;
        }
        let other = snapshot(PROPERTY, &kept, 1)?;
        call(PROPERTY, store.save(other.clone()).await)?;

        call(PROPERTY, store.forget(&forgotten).await)?;

        expect_latest(store, PROPERTY, &forgotten, None).await?;
        expect_latest(store, PROPERTY, &kept, Some(&other)).await?;

        // Forgetting an unknown stream is not an error: there is
        // nothing to drop, which is the state being asked for.
        call(PROPERTY, store.forget(&forgotten).await)
    }

    async fn receipt_adoptions_survive_snapshot_reopen(
        store: &dyn CeremonySnapshotStorePort,
    ) -> Result<(), ConformanceFailure> {
        const PROPERTY: &str = "receipt_adoptions_survive_snapshot_reopen";
        let stream = ceremony_id(PROPERTY, "receipt")?;
        let mut snapshot = snapshot(PROPERTY, &stream, 7)?;
        let operation = ExecutionOperationId::for_step(
            &stream,
            &StepId::new("conformance_step").map_err(|error| {
                failure(
                    PROPERTY,
                    format!("the suite built an invalid step id: {error}"),
                )
            })?,
            StateVisit::FIRST,
            StateIteration::FIRST,
            StepIteration::FIRST,
        );
        let producer = StepClaimFence::new("1".repeat(64)).map_err(|error| {
            failure(
                PROPERTY,
                format!("the suite built an invalid fence: {error}"),
            )
        })?;
        let applied = StepClaimFence::new("2".repeat(64)).map_err(|error| {
            failure(
                PROPERTY,
                format!("the suite built an invalid fence: {error}"),
            )
        })?;
        let second_applied = StepClaimFence::new("3".repeat(64)).map_err(|error| {
            failure(
                PROPERTY,
                format!("the suite built an invalid fence: {error}"),
            )
        })?;
        let receipt = ExecutionReceiptId::for_operation(&operation);
        let direct = receipt_event(
            operation.clone(),
            receipt.clone(),
            producer.clone(),
            producer.clone(),
            ExecutionReceiptLinkKind::Direct,
        )?;
        let adoption = receipt_event(
            operation.clone(),
            receipt.clone(),
            producer.clone(),
            applied.clone(),
            ExecutionReceiptLinkKind::Adopted,
        )?;
        let second_adoption = receipt_event(
            operation.clone(),
            receipt,
            producer,
            second_applied.clone(),
            ExecutionReceiptLinkKind::Adopted,
        )?;
        snapshot.instance.apply(&direct);
        snapshot.instance.apply(&adoption);
        snapshot.instance.apply(&second_adoption);

        call(PROPERTY, store.save(snapshot.clone()).await)?;
        let reopened = call(PROPERTY, store.latest(&stream).await)?
            .ok_or_else(|| failure(PROPERTY, "the receipt adoption snapshot disappeared"))?;
        if reopened.instance.execution_receipt_link(&operation)
            != snapshot.instance.execution_receipt_link(&operation)
            || reopened
                .instance
                .execution_receipt_adoption(&operation, &applied)
                != snapshot
                    .instance
                    .execution_receipt_adoption(&operation, &applied)
            || reopened
                .instance
                .execution_receipt_adoption(&operation, &second_applied)
                != snapshot
                    .instance
                    .execution_receipt_adoption(&operation, &second_applied)
        {
            return Err(failure(
                PROPERTY,
                "the original receipt link or its adoption changed after reopening",
            ));
        }
        Ok(())
    }
}

fn receipt_event(
    operation: ExecutionOperationId,
    receipt: ExecutionReceiptId,
    producer: StepClaimFence,
    applied: StepClaimFence,
    kind: ExecutionReceiptLinkKind,
) -> Result<CeremonyEvent, ConformanceFailure> {
    let link = ExecutionReceiptLink::new(receipt, operation, producer, applied, kind).map_err(
        |error| {
            failure(
                "receipt_adoptions_survive_snapshot_reopen",
                error.to_string(),
            )
        },
    )?;
    Ok(CeremonyEvent::ExecutionReceiptLinked(
        ExecutionReceiptLinked {
            step_id: StepId::new("conformance_step").expect("static step id"),
            link,
            linked_at: OffsetDateTime::UNIX_EPOCH,
        },
    ))
}

async fn expect_latest(
    store: &dyn CeremonySnapshotStorePort,
    property: &'static str,
    stream: &CeremonyId,
    wanted: Option<&CeremonySnapshot>,
) -> Result<(), ConformanceFailure> {
    let latest = call(property, store.latest(stream).await)?;
    if latest.as_ref() != wanted {
        return Err(failure(
            property,
            format!(
                "expected the latest snapshot at {:?}, found {:?}",
                wanted.map(|snapshot| snapshot.version),
                latest.map(|snapshot| snapshot.version)
            ),
        ));
    }
    Ok(())
}

fn call<T>(
    property: &'static str,
    outcome: Result<T, DomainError>,
) -> Result<T, ConformanceFailure> {
    outcome.map_err(|error| failure(property, format!("the adapter returned an error: {error}")))
}

fn failure(property: &'static str, detail: impl Into<String>) -> ConformanceFailure {
    ConformanceFailure::new(property, detail)
}

fn ceremony_id(property: &'static str, suffix: &str) -> Result<CeremonyId, ConformanceFailure> {
    CeremonyId::new(format!("conformance-{property}-{suffix}")).map_err(|error| {
        failure(
            property,
            format!("the suite built an invalid ceremony id: {error}"),
        )
    })
}

fn snapshot(
    property: &'static str,
    stream: &CeremonyId,
    version: u64,
) -> Result<CeremonySnapshot, ConformanceFailure> {
    let definition = definition().map_err(|error| {
        failure(
            property,
            format!("the suite built an invalid definition: {error}"),
        )
    })?;
    Ok(CeremonySnapshot {
        version: StreamVersion::new(version),
        instance: CeremonyInstance::start(
            stream.clone(),
            &definition,
            CeremonyContext::empty(),
            OffsetDateTime::UNIX_EPOCH,
        )
        .expect("required ceremony inputs"),
    })
}
