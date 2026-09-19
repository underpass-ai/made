use made_adapters::sqlite::{SqliteCouncilJournal, SqliteCouncilStore};
use made_core::entities::CouncilJournalEvent;
use made_core::events::{EventEnvelope, PhaseChangedEvent};
use made_core::ports::CouncilJournalPort;
use made_core::value_objects::{EventId, TaskId};
use made_tests_integration::parity_clock::PARITY_INSTANT;
use serde_json::{json, Value};
use std::path::Path;
use std::sync::Arc;

pub(super) fn scratch_directory(label: &str) -> tempfile::TempDir {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tmp");
    std::fs::create_dir_all(&root).unwrap();
    tempfile::Builder::new()
        .prefix(label)
        .tempdir_in(root)
        .unwrap()
}

pub(super) async fn seeded(directory: &Path) -> Arc<SqliteCouncilJournal> {
    let journal = Arc::new(SqliteCouncilJournal::new(
        SqliteCouncilStore::open(directory.join("councils.sqlite3")).unwrap(),
    ));
    let event = PhaseChangedEvent::new(
        EventEnvelope::new(
            EventId::new("parity-council-phase").unwrap(),
            PARITY_INSTANT,
            "parity",
            None,
        )
        .unwrap(),
        TaskId::new("parity-council-task").unwrap(),
        "proposing",
        "reviewing",
    )
    .unwrap();
    journal
        .publish(CouncilJournalEvent::PhaseChanged(event))
        .await
        .unwrap();
    journal
}

pub(super) fn script() -> Vec<(&'static str, Value)> {
    vec![
        ("made_read_council_events", json!({"limit":1})),
        (
            "made_get_council_event_cursor",
            json!({"consumer":"parity-council-ack"}),
        ),
        (
            "made_lease_council_events",
            json!({"consumer":"parity-council-ack","duration_ms":30_000}),
        ),
        (
            "made_acknowledge_council_events",
            json!({"lease":"$council-lease:parity-council-ack","through":1}),
        ),
        (
            "made_get_council_event_cursor",
            json!({"consumer":"parity-council-ack"}),
        ),
        (
            "made_lease_council_events",
            json!({"consumer":"parity-council-release","duration_ms":30_000}),
        ),
        (
            "made_release_council_events",
            json!({"lease":"$council-lease:parity-council-release"}),
        ),
        (
            "made_get_council_event_cursor",
            json!({"consumer":"parity-council-release"}),
        ),
    ]
}
