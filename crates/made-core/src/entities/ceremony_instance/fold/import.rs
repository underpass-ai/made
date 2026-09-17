use crate::entities::ceremony_events::InstanceImported;
use crate::entities::CeremonyInstance;

impl CeremonyInstance {
    /// The session the import carried, as it was carried.
    ///
    /// No derivation and no definition: an imported session is not
    /// opened, it is restored. What the legacy store held is what the
    /// stream now says, and the fold of the stream is that snapshot —
    /// which is exactly the equality the migration verifies before it
    /// installs anything.
    #[must_use]
    pub fn from_imported(imported: &InstanceImported) -> Self {
        imported.snapshot.as_ref().clone()
    }

    /// Replace the session with the one the import carried.
    ///
    /// The only event that overwrites rather than adds. It can only
    /// legitimately be the first record of a stream — `rehydrate`
    /// refuses it anywhere else — so nothing it could overwrite exists
    /// when it is applied in order.
    pub(super) fn apply_instance_imported(&mut self, imported: &InstanceImported) {
        *self = Self::from_imported(imported);
    }
}
