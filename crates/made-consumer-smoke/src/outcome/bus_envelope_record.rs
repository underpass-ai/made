/// Snapshot of a bus envelope observed on a subscription during a
/// chain run. Captures only the fields the harness asserts on.
#[derive(Debug, Clone)]
pub struct BusEnvelopeRecord {
    pub subject: String,
    pub event_id: String,
    pub correlation_id: Option<String>,
    pub causation_id: Option<String>,
}
