/// Operator-visible TLS posture options.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MadeMcpGrpcTlsMode {
    Disabled,
    Server,
    Mutual,
}

impl MadeMcpGrpcTlsMode {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Disabled => "disabled",
            Self::Server => "server",
            Self::Mutual => "mutual",
        }
    }
}
