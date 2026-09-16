//! Conformance suite for [`CeremonyEventStorePort`].
//!
//! A stream is a claim about order, contiguity and exclusivity at the
//! same time, so every property here reads the stream back after the
//! write it exercised. An adapter that lands records but files them
//! out of order, or refuses a stale append yet lets half of it in, is
//! exactly what this suite exists to catch.
//!
//! # What this suite cannot check
//!
//! **Crash atomicity.** A rejected batch leaves nothing behind when
//! the process survives; whether a batch interrupted mid-write leaves
//! nothing behind is a property of the store, and the host proves it
//! against its own.

use futures::future::join_all;

use crate::entities::{AuditChain, AuditFact};
use crate::error::DomainError;
use crate::ports::{AppendOutcome, CeremonyEventStorePort};
use crate::value_objects::{CeremonyId, GlobalPosition, StreamVersion};

use super::conformance_fixtures::{audit_fact, definition};
use super::ConformanceFailure;

const CONCURRENT_APPENDS: usize = 8;

/// Every property a [`CeremonyEventStorePort`] implementation must
/// satisfy.
#[derive(Debug)]
pub struct CeremonyEventStoreConformance;

impl CeremonyEventStoreConformance {
    /// Run the whole suite against one store. Each property uses its
    /// own streams, so a store other properties wrote to is fine.
    pub async fn run(
        store: &dyn CeremonyEventStorePort,
    ) -> Result<Vec<&'static str>, ConformanceFailure> {
        let mut passed = Vec::new();
        Self::an_unknown_stream_is_empty(store).await?;
        passed.push("an_unknown_stream_is_empty");
        Self::a_first_append_lands_contiguous_sequences(store).await?;
        passed.push("a_first_append_lands_contiguous_sequences");
        Self::a_stale_expectation_conflicts_and_writes_nothing(store).await?;
        passed.push("a_stale_expectation_conflicts_and_writes_nothing");
        Self::successive_appends_form_one_intact_chain(store).await?;
        passed.push("successive_appends_form_one_intact_chain");
        Self::a_duplicate_event_id_is_refused_across_appends(store).await?;
        passed.push("a_duplicate_event_id_is_refused_across_appends");
        Self::a_duplicate_event_id_is_refused_within_one_batch(store).await?;
        passed.push("a_duplicate_event_id_is_refused_within_one_batch");
        Self::a_fact_from_another_ceremony_is_refused(store).await?;
        passed.push("a_fact_from_another_ceremony_is_refused");
        Self::an_empty_batch_is_refused(store).await?;
        passed.push("an_empty_batch_is_refused");
        Self::concurrent_appends_admit_exactly_one_winner(store).await?;
        passed.push("concurrent_appends_admit_exactly_one_winner");
        Self::read_after_a_version_returns_only_later_records(store).await?;
        passed.push("read_after_a_version_returns_only_later_records");
        Self::read_all_follows_append_order_across_streams(store).await?;
        passed.push("read_all_follows_append_order_across_streams");
        Self::read_all_honours_from_and_limit_and_is_stable(store).await?;
        passed.push("read_all_honours_from_and_limit_and_is_stable");
        Self::streams_lists_each_stream_once_sorted(store).await?;
        passed.push("streams_lists_each_stream_once_sorted");
        Ok(passed)
    }

    async fn an_unknown_stream_is_empty(
        store: &dyn CeremonyEventStorePort,
    ) -> Result<(), ConformanceFailure> {
        const PROPERTY: &str = "an_unknown_stream_is_empty";
        let stream = ceremony_id(PROPERTY, "unknown")?;

        let head = call(PROPERTY, store.head(&stream).await)?;
        if head != StreamVersion::EMPTY {
            return Err(failure(
                PROPERTY,
                format!("a stream nothing was appended to has head {head:?}"),
            ));
        }
        let records = call(PROPERTY, store.read(&stream, StreamVersion::EMPTY).await)?;
        if !records.is_empty() {
            return Err(failure(PROPERTY, "an unknown stream returned records"));
        }
        if call(PROPERTY, store.streams().await)?.contains(&stream) {
            return Err(failure(PROPERTY, "an unknown stream is listed"));
        }
        Ok(())
    }

    async fn a_first_append_lands_contiguous_sequences(
        store: &dyn CeremonyEventStorePort,
    ) -> Result<(), ConformanceFailure> {
        const PROPERTY: &str = "a_first_append_lands_contiguous_sequences";
        let stream = ceremony_id(PROPERTY, "first")?;

        let outcome = append(store, PROPERTY, &stream, StreamVersion::EMPTY, 1..=3).await?;
        let AppendOutcome::Appended {
            version, records, ..
        } = outcome
        else {
            return Err(failure(PROPERTY, "a first append conflicted"));
        };
        if version != StreamVersion::new(3) {
            return Err(failure(
                PROPERTY,
                format!("three facts left the stream at version {version:?}"),
            ));
        }
        expect_sequences(PROPERTY, &records, 1..=3)?;

        let stored = call(PROPERTY, store.read(&stream, StreamVersion::EMPTY).await)?;
        if stored != records {
            return Err(failure(
                PROPERTY,
                "the records read back differ from the ones reported as appended",
            ));
        }
        expect_head(store, PROPERTY, &stream, 3).await
    }

    /// The property the expectation exists for: a rejected batch must
    /// not leave any of itself behind.
    async fn a_stale_expectation_conflicts_and_writes_nothing(
        store: &dyn CeremonyEventStorePort,
    ) -> Result<(), ConformanceFailure> {
        const PROPERTY: &str = "a_stale_expectation_conflicts_and_writes_nothing";
        let stream = ceremony_id(PROPERTY, "stale")?;

        append(store, PROPERTY, &stream, StreamVersion::EMPTY, 1..=2).await?;
        let before = call(PROPERTY, store.read(&stream, StreamVersion::EMPTY).await)?;

        let outcome = append(store, PROPERTY, &stream, StreamVersion::EMPTY, 3..=4).await?;
        match outcome {
            AppendOutcome::Conflict { expected, actual }
                if expected == StreamVersion::EMPTY && actual == StreamVersion::new(2) => {}
            AppendOutcome::Conflict { expected, actual } => {
                return Err(failure(
                    PROPERTY,
                    format!("the conflict reports expected {expected:?}, actual {actual:?}"),
                ))
            }
            AppendOutcome::Appended { .. } => {
                return Err(failure(
                    PROPERTY,
                    "an append decided against an empty stream was accepted over two records",
                ))
            }
        }

        let after = call(PROPERTY, store.read(&stream, StreamVersion::EMPTY).await)?;
        if after != before {
            return Err(failure(
                PROPERTY,
                "a rejected append still changed the stream",
            ));
        }
        expect_head(store, PROPERTY, &stream, 2).await
    }

    async fn successive_appends_form_one_intact_chain(
        store: &dyn CeremonyEventStorePort,
    ) -> Result<(), ConformanceFailure> {
        const PROPERTY: &str = "successive_appends_form_one_intact_chain";
        let stream = ceremony_id(PROPERTY, "chaining")?;

        let mut expected = StreamVersion::EMPTY;
        for batch in 0..3_u64 {
            let first = batch * 2 + 1;
            let outcome = append(store, PROPERTY, &stream, expected, first..=first + 1).await?;
            expected = outcome.appended_version().ok_or_else(|| {
                failure(PROPERTY, format!("append {batch} conflicted unexpectedly"))
            })?;
        }

        let records = call(PROPERTY, store.read(&stream, StreamVersion::EMPTY).await)?;
        expect_sequences(PROPERTY, &records, 1..=6)?;
        let verdict = AuditChain::verify(&records);
        if !verdict.is_intact() {
            return Err(failure(
                PROPERTY,
                format!(
                    "records sealed across appends do not chain: {:?}",
                    verdict.defect()
                ),
            ));
        }
        expect_head(store, PROPERTY, &stream, 6).await
    }

    async fn a_duplicate_event_id_is_refused_across_appends(
        store: &dyn CeremonyEventStorePort,
    ) -> Result<(), ConformanceFailure> {
        const PROPERTY: &str = "a_duplicate_event_id_is_refused_across_appends";
        let stream = ceremony_id(PROPERTY, "replayed")?;

        append(store, PROPERTY, &stream, StreamVersion::EMPTY, 1..=1).await?;
        // A fresh fact first, the replayed one second: the whole batch
        // must be refused, not just the offending fact.
        let batch = vec![fact(PROPERTY, &stream, 2)?, fact(PROPERTY, &stream, 1)?];
        expect_refused(
            PROPERTY,
            store.append(&stream, StreamVersion::new(1), batch).await,
            "an event id the stream already holds",
        )?;
        expect_head(store, PROPERTY, &stream, 1).await
    }

    async fn a_duplicate_event_id_is_refused_within_one_batch(
        store: &dyn CeremonyEventStorePort,
    ) -> Result<(), ConformanceFailure> {
        const PROPERTY: &str = "a_duplicate_event_id_is_refused_within_one_batch";
        let stream = ceremony_id(PROPERTY, "twice")?;

        let batch = vec![
            fact(PROPERTY, &stream, 1)?,
            fact(PROPERTY, &stream, 2)?,
            fact(PROPERTY, &stream, 1)?,
        ];
        expect_refused(
            PROPERTY,
            store.append(&stream, StreamVersion::EMPTY, batch).await,
            "an event id that occurs twice in one batch",
        )?;
        expect_head(store, PROPERTY, &stream, 0).await
    }

    async fn a_fact_from_another_ceremony_is_refused(
        store: &dyn CeremonyEventStorePort,
    ) -> Result<(), ConformanceFailure> {
        const PROPERTY: &str = "a_fact_from_another_ceremony_is_refused";
        let stream = ceremony_id(PROPERTY, "own")?;
        let other = ceremony_id(PROPERTY, "other")?;

        let batch = vec![fact(PROPERTY, &stream, 1)?, fact(PROPERTY, &other, 2)?];
        expect_refused(
            PROPERTY,
            store.append(&stream, StreamVersion::EMPTY, batch).await,
            "a fact belonging to another ceremony",
        )?;
        expect_head(store, PROPERTY, &stream, 0).await?;
        expect_head(store, PROPERTY, &other, 0).await
    }

    async fn an_empty_batch_is_refused(
        store: &dyn CeremonyEventStorePort,
    ) -> Result<(), ConformanceFailure> {
        const PROPERTY: &str = "an_empty_batch_is_refused";
        let stream = ceremony_id(PROPERTY, "empty")?;

        expect_refused(
            PROPERTY,
            store
                .append(&stream, StreamVersion::EMPTY, Vec::new())
                .await,
            "an empty batch",
        )?;
        expect_head(store, PROPERTY, &stream, 0).await
    }

    async fn concurrent_appends_admit_exactly_one_winner(
        store: &dyn CeremonyEventStorePort,
    ) -> Result<(), ConformanceFailure> {
        const PROPERTY: &str = "concurrent_appends_admit_exactly_one_winner";
        let stream = ceremony_id(PROPERTY, "racing")?;

        let mut pending = Vec::with_capacity(CONCURRENT_APPENDS);
        for ordinal in 1..=CONCURRENT_APPENDS as u64 {
            let batch = vec![fact(PROPERTY, &stream, ordinal)?];
            pending.push(store.append(&stream, StreamVersion::EMPTY, batch));
        }

        let mut appended = 0;
        for outcome in join_all(pending).await {
            if !call(PROPERTY, outcome)?.is_conflict() {
                appended += 1;
            }
        }
        if appended != 1 {
            return Err(failure(
                PROPERTY,
                format!("{appended} concurrent appends were accepted, expected exactly 1"),
            ));
        }
        expect_head(store, PROPERTY, &stream, 1).await
    }

    async fn read_after_a_version_returns_only_later_records(
        store: &dyn CeremonyEventStorePort,
    ) -> Result<(), ConformanceFailure> {
        const PROPERTY: &str = "read_after_a_version_returns_only_later_records";
        let stream = ceremony_id(PROPERTY, "tail")?;

        append(store, PROPERTY, &stream, StreamVersion::EMPTY, 1..=5).await?;

        let tail = call(PROPERTY, store.read(&stream, StreamVersion::new(3)).await)?;
        expect_sequences(PROPERTY, &tail, 4..=5)?;
        let nothing = call(PROPERTY, store.read(&stream, StreamVersion::new(5)).await)?;
        if !nothing.is_empty() {
            return Err(failure(PROPERTY, "reading after the head returned records"));
        }
        let beyond = call(PROPERTY, store.read(&stream, StreamVersion::new(9)).await)?;
        if !beyond.is_empty() {
            return Err(failure(
                PROPERTY,
                "reading after a version beyond the head returned records",
            ));
        }
        Ok(())
    }

    async fn read_all_follows_append_order_across_streams(
        store: &dyn CeremonyEventStorePort,
    ) -> Result<(), ConformanceFailure> {
        const PROPERTY: &str = "read_all_follows_append_order_across_streams";
        let left = ceremony_id(PROPERTY, "left")?;
        let right = ceremony_id(PROPERTY, "right")?;

        // left 1-2, right 1, left 3: the global order interleaves the
        // streams exactly as the appends happened.
        append(store, PROPERTY, &left, StreamVersion::EMPTY, 1..=2).await?;
        append(store, PROPERTY, &right, StreamVersion::EMPTY, 1..=1).await?;
        append(store, PROPERTY, &left, StreamVersion::new(2), 3..=3).await?;

        let rows = call(
            PROPERTY,
            store.read_all(GlobalPosition::FIRST, usize::MAX).await,
        )?;
        if !rows
            .windows(2)
            .all(|pair| pair[0].position < pair[1].position)
        {
            return Err(failure(PROPERTY, "positions are not strictly increasing"));
        }
        let ours: Vec<(&CeremonyId, u64)> = rows
            .iter()
            .filter(|row| [&left, &right].contains(&row.record.ceremony_id()))
            .map(|row| (row.record.ceremony_id(), row.record.sequence().value()))
            .collect();
        let wanted = [(&left, 1), (&left, 2), (&right, 1), (&left, 3)];
        if ours != wanted {
            return Err(failure(
                PROPERTY,
                format!("the global order does not follow append order: {ours:?}"),
            ));
        }
        Ok(())
    }

    async fn read_all_honours_from_and_limit_and_is_stable(
        store: &dyn CeremonyEventStorePort,
    ) -> Result<(), ConformanceFailure> {
        const PROPERTY: &str = "read_all_honours_from_and_limit_and_is_stable";
        let stream = ceremony_id(PROPERTY, "paged")?;

        append(store, PROPERTY, &stream, StreamVersion::EMPTY, 1..=4).await?;

        let all = call(
            PROPERTY,
            store.read_all(GlobalPosition::FIRST, usize::MAX).await,
        )?;
        let again = call(
            PROPERTY,
            store.read_all(GlobalPosition::FIRST, usize::MAX).await,
        )?;
        if all != again {
            return Err(failure(PROPERTY, "two consecutive reads differ"));
        }
        let Some(second) = all.get(1) else {
            return Err(failure(PROPERTY, "fewer than two rows after four appends"));
        };

        let page = call(PROPERTY, store.read_all(second.position, 2).await)?;
        if page.len() != 2 {
            return Err(failure(
                PROPERTY,
                format!("a limit of 2 returned {} rows", page.len()),
            ));
        }
        if page[0] != *second {
            return Err(failure(PROPERTY, "`from` is not inclusive"));
        }
        if page[1] != all[2] {
            return Err(failure(
                PROPERTY,
                "a page does not continue the global order",
            ));
        }
        let empty = call(PROPERTY, store.read_all(GlobalPosition::FIRST, 0).await)?;
        if !empty.is_empty() {
            return Err(failure(PROPERTY, "a limit of 0 returned rows"));
        }
        Ok(())
    }

    async fn streams_lists_each_stream_once_sorted(
        store: &dyn CeremonyEventStorePort,
    ) -> Result<(), ConformanceFailure> {
        const PROPERTY: &str = "streams_lists_each_stream_once_sorted";
        let ids = [
            ceremony_id(PROPERTY, "c")?,
            ceremony_id(PROPERTY, "a")?,
            ceremony_id(PROPERTY, "b")?,
        ];

        for stream in &ids {
            append(store, PROPERTY, stream, StreamVersion::EMPTY, 1..=2).await?;
        }
        append(store, PROPERTY, &ids[1], StreamVersion::new(2), 3..=3).await?;

        let listed = call(PROPERTY, store.streams().await)?;
        for stream in &ids {
            let times = listed.iter().filter(|id| *id == stream).count();
            if times != 1 {
                return Err(failure(
                    PROPERTY,
                    format!("{} is listed {times} times", stream.as_str()),
                ));
            }
        }
        if !listed.windows(2).all(|pair| pair[0] < pair[1]) {
            return Err(failure(PROPERTY, "streams are not sorted by id"));
        }
        Ok(())
    }
}

async fn append(
    store: &dyn CeremonyEventStorePort,
    property: &'static str,
    stream: &CeremonyId,
    expected: StreamVersion,
    ordinals: std::ops::RangeInclusive<u64>,
) -> Result<AppendOutcome, ConformanceFailure> {
    let facts = ordinals
        .map(|ordinal| fact(property, stream, ordinal))
        .collect::<Result<Vec<_>, _>>()?;
    call(property, store.append(stream, expected, facts).await)
}

async fn expect_head(
    store: &dyn CeremonyEventStorePort,
    property: &'static str,
    stream: &CeremonyId,
    version: u64,
) -> Result<(), ConformanceFailure> {
    let head = call(property, store.head(stream).await)?;
    if head != StreamVersion::new(version) {
        return Err(failure(
            property,
            format!("expected the head at {version}, found {}", head.value()),
        ));
    }
    Ok(())
}

fn expect_sequences(
    property: &'static str,
    records: &[crate::entities::AuditRecord],
    wanted: std::ops::RangeInclusive<u64>,
) -> Result<(), ConformanceFailure> {
    let found: Vec<u64> = records
        .iter()
        .map(|record| record.sequence().value())
        .collect();
    if found != wanted.clone().collect::<Vec<_>>() {
        return Err(failure(
            property,
            format!("expected sequences {wanted:?}, found {found:?}"),
        ));
    }
    Ok(())
}

fn expect_refused(
    property: &'static str,
    outcome: Result<AppendOutcome, DomainError>,
    what: &str,
) -> Result<(), ConformanceFailure> {
    match outcome {
        Err(_) => Ok(()),
        Ok(outcome) => Err(failure(
            property,
            format!("{what} was not refused as an error: {outcome:?}"),
        )),
    }
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

fn fact(
    property: &'static str,
    stream: &CeremonyId,
    ordinal: u64,
) -> Result<AuditFact, ConformanceFailure> {
    let build = || audit_fact(&format!("{property}-{ordinal}"), stream, &definition()?);
    build().map_err(|error: DomainError| {
        failure(
            property,
            format!("the suite built an invalid fact: {error}"),
        )
    })
}
