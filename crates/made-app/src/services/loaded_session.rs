use made_core::entities::CeremonyInstance;
use made_core::value_objects::{EventId, StreamVersion};

/// A ceremony instance as folded from its stream, with the version it
/// was folded to.
///
/// The version is what the next append is decided against; the head
/// event id is what the next fact names as its cause. Both are read
/// once, at load, so committing needs nothing the session does not
/// already carry.
#[derive(Debug)]
pub struct LoadedSession {
    pub instance: CeremonyInstance,
    pub version: StreamVersion,
    head: Option<EventId>,
}

impl LoadedSession {
    pub(crate) fn new(
        instance: CeremonyInstance,
        version: StreamVersion,
        head: Option<EventId>,
    ) -> Self {
        Self {
            instance,
            version,
            head,
        }
    }

    /// The id of the last record in the stream at load time: what a
    /// fact decided against this session was caused by.
    #[must_use]
    pub fn head_event_id(&self) -> Option<&EventId> {
        self.head.as_ref()
    }

    pub(crate) fn into_parts(self) -> (CeremonyInstance, StreamVersion, Option<EventId>) {
        (self.instance, self.version, self.head)
    }
}
