/// The operational metrics recorder selected by a composition root.
///
/// Names are static because recorder implementations are fixed by the build,
/// while the type keeps provider vocabulary out of the metrics port contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RecorderName(&'static str);

impl RecorderName {
    #[must_use]
    pub const fn new(value: &'static str) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        self.0
    }
}
