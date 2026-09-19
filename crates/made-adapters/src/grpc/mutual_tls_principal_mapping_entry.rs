use made_core::value_objects::{
    AuthenticatedPrincipal, AuthenticationMethod, PrincipalId, PrincipalKind,
};
use made_core::DomainError;
use serde::{Deserialize, Serialize};

/// One explicitly trusted client certificate in the gRPC mTLS boundary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MutualTlsPrincipalMappingEntry {
    certificate_sha256: String,
    principal_id: PrincipalId,
    principal_kind: PrincipalKind,
}

impl MutualTlsPrincipalMappingEntry {
    pub fn new(
        certificate_sha256: impl Into<String>,
        principal_id: PrincipalId,
        principal_kind: PrincipalKind,
    ) -> Result<Self, DomainError> {
        let entry = Self {
            certificate_sha256: certificate_sha256.into(),
            principal_id,
            principal_kind,
        };
        entry.validate()?;
        Ok(entry)
    }

    #[must_use]
    pub fn certificate_sha256(&self) -> &str {
        &self.certificate_sha256
    }

    pub fn principal(&self) -> Result<AuthenticatedPrincipal, DomainError> {
        AuthenticatedPrincipal::new(
            self.principal_id.clone(),
            self.principal_kind,
            AuthenticationMethod::MutualTls,
        )
    }

    pub(crate) fn validate(&self) -> Result<(), DomainError> {
        let fingerprint = self.certificate_sha256.as_bytes();
        if fingerprint.len() != 64
            || !fingerprint
                .iter()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
        {
            return Err(DomainError::InvariantViolated {
                reason: "mTLS certificate fingerprint must be lowercase SHA-256 hex",
            });
        }
        self.principal().map(|_| ())
    }
}
