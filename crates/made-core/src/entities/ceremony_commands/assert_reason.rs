use crate::value_objects::CeremonyReason;

/// State why one thing in this session led to another.
///
/// The reason is the command: it already carries both ends, the kind,
/// the why, the confidence, the seat asserting it and when. The seat
/// is `asserted_by`, and a reason with no seat is refused — the only
/// reasons the engine asserts on its own are recorded by the fold
/// beside the response they describe, never decided.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssertReason {
    pub reason: CeremonyReason,
}
