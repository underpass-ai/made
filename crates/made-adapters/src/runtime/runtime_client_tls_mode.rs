/// Operator-visible TLS posture options for the Runtime client.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeClientTlsMode {
    Disabled,
    Server,
    Mutual,
}

impl RuntimeClientTlsMode {
    /// Stable label for logs/metadata.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Disabled => "disabled",
            Self::Server => "server",
            Self::Mutual => "mutual",
        }
    }
}
