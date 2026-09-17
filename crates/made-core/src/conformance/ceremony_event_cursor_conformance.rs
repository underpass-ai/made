use time::{Duration, OffsetDateTime};

use crate::error::DomainError;
use crate::ports::CeremonyEventCursorPort;
use crate::value_objects::{
    CeremonyEventConsumer, CeremonyEventCursorLease, CeremonyEventCursorLeaseId,
    CeremonyEventQuarantineReason, DurationMs, GlobalPosition,
};

use super::ConformanceFailure;

const LEASE_MILLIS: u64 = 30_000;

/// Storage-independent properties of a durable named event cursor.
#[derive(Debug)]
pub struct CeremonyEventCursorConformance;

impl CeremonyEventCursorConformance {
    pub async fn run(
        cursor: &dyn CeremonyEventCursorPort,
    ) -> Result<Vec<&'static str>, ConformanceFailure> {
        let mut passed = Vec::new();
        Self::independent_consumers_advance_independently(cursor).await?;
        passed.push("independent_consumers_advance_independently");
        Self::acknowledged_progress_resumes(cursor).await?;
        passed.push("acknowledged_progress_resumes");
        Self::a_live_lease_excludes_another_worker(cursor).await?;
        passed.push("a_live_lease_excludes_another_worker");
        Self::an_expired_lease_can_be_replaced(cursor).await?;
        passed.push("an_expired_lease_can_be_replaced");
        Self::failures_retry_and_quarantine_is_visible(cursor).await?;
        passed.push("failures_retry_and_quarantine_is_visible");
        Ok(passed)
    }

    async fn independent_consumers_advance_independently(
        cursor: &dyn CeremonyEventCursorPort,
    ) -> Result<(), ConformanceFailure> {
        const PROPERTY: &str = "independent_consumers_advance_independently";
        let left = consumer(PROPERTY, "left")?;
        let right = consumer(PROPERTY, "right")?;
        call(
            PROPERTY,
            cursor
                .acknowledge(&left, GlobalPosition::new(3).expect("valid position"))
                .await,
        )?;
        call(
            PROPERTY,
            cursor
                .acknowledge(&right, GlobalPosition::new(7).expect("valid position"))
                .await,
        )?;
        if call(PROPERTY, cursor.position(&left).await)?
            != Some(GlobalPosition::new(3).expect("valid position"))
            || call(PROPERTY, cursor.position(&right).await)?
                != Some(GlobalPosition::new(7).expect("valid position"))
        {
            return Err(failure(PROPERTY, "one named cursor changed another"));
        }
        Ok(())
    }

    async fn acknowledged_progress_resumes(
        cursor: &dyn CeremonyEventCursorPort,
    ) -> Result<(), ConformanceFailure> {
        const PROPERTY: &str = "acknowledged_progress_resumes";
        let consumer = consumer(PROPERTY, "consumer")?;
        let through = GlobalPosition::new(11).expect("valid position");
        call(PROPERTY, cursor.acknowledge(&consumer, through).await)?;
        let lease = acquire(PROPERTY, cursor, &consumer, "resume", now())
            .await?
            .ok_or_else(|| failure(PROPERTY, "an unleased cursor could not be leased"))?;
        if lease.acknowledged_through() != Some(through) || lease.next_position() != through.next()
        {
            return Err(failure(
                PROPERTY,
                "a later worker did not resume after acknowledged progress",
            ));
        }
        call(PROPERTY, cursor.release(&lease).await)
    }

    async fn a_live_lease_excludes_another_worker(
        cursor: &dyn CeremonyEventCursorPort,
    ) -> Result<(), ConformanceFailure> {
        const PROPERTY: &str = "a_live_lease_excludes_another_worker";
        let consumer = consumer(PROPERTY, "consumer")?;
        let first = acquire(PROPERTY, cursor, &consumer, "first", now())
            .await?
            .ok_or_else(|| failure(PROPERTY, "the first worker could not lease"))?;
        let second = acquire(PROPERTY, cursor, &consumer, "second", now()).await?;
        if second.is_some() {
            return Err(failure(PROPERTY, "a live lease was handed to two workers"));
        }
        call(PROPERTY, cursor.release(&first).await)
    }

    async fn an_expired_lease_can_be_replaced(
        cursor: &dyn CeremonyEventCursorPort,
    ) -> Result<(), ConformanceFailure> {
        const PROPERTY: &str = "an_expired_lease_can_be_replaced";
        let consumer = consumer(PROPERTY, "consumer")?;
        acquire(PROPERTY, cursor, &consumer, "first", now())
            .await?
            .ok_or_else(|| failure(PROPERTY, "the first worker could not lease"))?;
        let after_expiry = now() + Duration::milliseconds(LEASE_MILLIS as i64 + 1);
        let replacement = acquire(PROPERTY, cursor, &consumer, "replacement", after_expiry)
            .await?
            .ok_or_else(|| failure(PROPERTY, "an expired lease stranded the cursor"))?;
        call(PROPERTY, cursor.release(&replacement).await)
    }

    async fn failures_retry_and_quarantine_is_visible(
        cursor: &dyn CeremonyEventCursorPort,
    ) -> Result<(), ConformanceFailure> {
        const PROPERTY: &str = "failures_retry_and_quarantine_is_visible";
        let consumer = consumer(PROPERTY, "consumer")?;
        let position = GlobalPosition::FIRST;
        let first = acquire(PROPERTY, cursor, &consumer, "failure-1", now())
            .await?
            .ok_or_else(|| failure(PROPERTY, "the cursor could not be leased"))?;
        call(PROPERTY, cursor.mark_failed(&first, position).await)?;
        let retry = acquire(PROPERTY, cursor, &consumer, "failure-2", now())
            .await?
            .ok_or_else(|| failure(PROPERTY, "a failed position was not retryable"))?;
        if retry.attempt().value() != 1 || retry.next_position() != position {
            return Err(failure(
                PROPERTY,
                "a retry did not retain its position and attempt count",
            ));
        }
        let reason = CeremonyEventQuarantineReason::new("conformance exhausted")
            .map_err(|error| failure(PROPERTY, error.to_string()))?;
        call(
            PROPERTY,
            cursor.quarantine(&retry, position, reason, now()).await,
        )?;
        if call(PROPERTY, cursor.position(&consumer).await)? != Some(position) {
            return Err(failure(PROPERTY, "quarantine did not advance the cursor"));
        }
        let quarantined = call(PROPERTY, cursor.quarantined(&consumer).await)?;
        if quarantined.len() != 1
            || quarantined[0].position() != position
            || quarantined[0].attempts().value() != 1
        {
            return Err(failure(PROPERTY, "quarantine was not recorded visibly"));
        }
        Ok(())
    }
}

async fn acquire(
    property: &'static str,
    cursor: &dyn CeremonyEventCursorPort,
    consumer: &CeremonyEventConsumer,
    suffix: &str,
    now: OffsetDateTime,
) -> Result<Option<CeremonyEventCursorLease>, ConformanceFailure> {
    let lease_id = CeremonyEventCursorLeaseId::new(format!("{property}-{suffix}"))
        .map_err(|error| failure(property, error.to_string()))?;
    call(
        property,
        cursor
            .lease(
                consumer,
                lease_id,
                now,
                DurationMs::from_millis(LEASE_MILLIS),
            )
            .await,
    )
}

fn consumer(
    property: &'static str,
    suffix: &str,
) -> Result<CeremonyEventConsumer, ConformanceFailure> {
    CeremonyEventConsumer::new(format!("{property}-{suffix}"))
        .map_err(|error| failure(property, error.to_string()))
}

fn now() -> OffsetDateTime {
    OffsetDateTime::UNIX_EPOCH
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
