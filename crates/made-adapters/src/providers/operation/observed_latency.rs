use super::Observed;
use made_core::value_objects::DurationMs;
/// Latency with explicit declared/external provenance.
pub type ObservedLatency = Observed<DurationMs>;
