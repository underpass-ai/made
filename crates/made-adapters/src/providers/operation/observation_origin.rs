use super::{ExternalAuthorityId, ObservationDeclarer};

/// Provenance of a latency, usage or finish/error observation.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ObservationOrigin {
    Declared { by: ObservationDeclarer },
    ExternalAuthority { authority: ExternalAuthorityId },
}
