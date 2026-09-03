use time::OffsetDateTime;

use crate::conformance::MemoryConformanceFailure;
use crate::ports::MemoryRecollection;
use crate::value_objects::{
    Attributes, CeremonyId, MemoryEntry, MemoryEntryId, MemoryEntryKind, MemoryProvenance,
    MemoryScope, MemoryWrite,
};

pub(super) type Checked = Result<(), MemoryConformanceFailure>;

pub(super) fn scope(name: &str) -> MemoryScope {
    MemoryScope::new(format!("ceremony:conformance-{name}")).expect("scope should be valid")
}

pub(super) fn entry(
    summary: &str,
    kind: MemoryEntryKind,
    observed_at: OffsetDateTime,
) -> MemoryEntry {
    named(summary, summary, kind, observed_at)
}

pub(super) fn named(
    id: &str,
    summary: &str,
    kind: MemoryEntryKind,
    observed_at: OffsetDateTime,
) -> MemoryEntry {
    MemoryEntry::new(
        MemoryEntryId::new(id).expect("entry id should be valid"),
        kind,
        summary,
        None,
        MemoryProvenance::new(
            CeremonyId::new("conformance").expect("ceremony id should be valid"),
            None,
            observed_at,
        ),
        Attributes::empty(),
    )
    .expect("entry should be valid")
}

pub(super) fn write(entries: Vec<MemoryEntry>) -> MemoryWrite {
    MemoryWrite::unexplained(entries).expect("a write with entries should be valid")
}

pub(super) fn moment(seconds: i64) -> OffsetDateTime {
    OffsetDateTime::UNIX_EPOCH + time::Duration::seconds(seconds)
}

pub(super) fn expect_unsupported(
    property: &'static str,
    recollection: &MemoryRecollection,
    operation: &str,
) -> Checked {
    if recollection.is_supported() {
        Err(MemoryConformanceFailure::new(
            property,
            format!("`{operation}` answered as supported by a backend that does not declare it"),
        ))
    } else {
        Ok(())
    }
}
