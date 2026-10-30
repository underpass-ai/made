use made_core::value_objects::ProtocolId;

/// What one run of the import brought forward.
///
/// The ids rather than a count: an operator asked to trust a one-way
/// migration is owed the names of the sessions it touched, and a test
/// that only compared counts would pass on the wrong two.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PreStreamImport {
    imported: Vec<ProtocolId>,
}

impl PreStreamImport {
    #[must_use]
    pub fn new(imported: Vec<ProtocolId>) -> Self {
        Self { imported }
    }

    /// The sessions that now have a stream, in the order they were
    /// imported.
    #[must_use]
    pub fn imported(&self) -> &[ProtocolId] {
        &self.imported
    }

    /// Nothing was imported, which on a second run means the store was
    /// already whole.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.imported.is_empty()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.imported.len()
    }
}
