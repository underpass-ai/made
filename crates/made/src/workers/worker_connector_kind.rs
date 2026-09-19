use made_core::DomainError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WorkerConnectorKind {
    Oci,
    Git,
    Http,
}

impl WorkerConnectorKind {
    pub(crate) fn parse(value: &str) -> Result<Self, DomainError> {
        match value {
            "oci" => Ok(Self::Oci),
            "git" => Ok(Self::Git),
            "http" => Ok(Self::Http),
            _ => Err(DomainError::InvalidDocument {
                reason: "MADE_WORKER_CONNECTOR must be oci, git, or http".to_owned(),
            }),
        }
    }

    pub(crate) const fn id(self) -> &'static str {
        match self {
            Self::Oci => "oci",
            Self::Git => "git",
            Self::Http => "http",
        }
    }
}
