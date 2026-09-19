use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{Context, Result};
use made_core::value_objects::AuthenticatedPrincipal;
use sha2::{Digest, Sha256};
use tonic::Request;

use super::{MutualTlsAuthenticationError, MutualTlsPrincipalMappingEntry};

/// Exact client-certificate fingerprints trusted by the gRPC boundary.
#[derive(Debug, Clone)]
pub struct MutualTlsPrincipalMap {
    principals: BTreeMap<String, AuthenticatedPrincipal>,
}

impl MutualTlsPrincipalMap {
    pub fn from_path(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let bytes = std::fs::read(path)
            .with_context(|| format!("failed to read mTLS principal map at {}", path.display()))?;
        Self::from_json(&bytes)
            .with_context(|| format!("invalid mTLS principal map at {}", path.display()))
    }

    pub fn from_json(bytes: &[u8]) -> Result<Self> {
        let entries: Vec<MutualTlsPrincipalMappingEntry> =
            serde_json::from_slice(bytes).context("principal map must be a JSON array")?;
        if entries.is_empty() {
            anyhow::bail!("mTLS principal map must contain at least one certificate");
        }
        let mut principals = BTreeMap::new();
        for entry in entries {
            entry.validate().map_err(anyhow::Error::from)?;
            let fingerprint = entry.certificate_sha256().to_owned();
            let principal = entry.principal().map_err(anyhow::Error::from)?;
            if principals.insert(fingerprint, principal).is_some() {
                anyhow::bail!("mTLS principal map contains a duplicate certificate fingerprint");
            }
        }
        Ok(Self { principals })
    }

    pub fn authenticate<T>(
        &self,
        request: &Request<T>,
    ) -> Result<AuthenticatedPrincipal, MutualTlsAuthenticationError> {
        let certificates = request
            .peer_certs()
            .ok_or(MutualTlsAuthenticationError::MissingCertificate)?;
        let leaf = certificates
            .first()
            .ok_or(MutualTlsAuthenticationError::EmptyCertificateChain)?;
        self.principal_for_der(leaf.as_ref())
    }

    pub fn principal_for_der(
        &self,
        certificate_der: &[u8],
    ) -> Result<AuthenticatedPrincipal, MutualTlsAuthenticationError> {
        let fingerprint = format!("{:x}", Sha256::digest(certificate_der));
        self.principals
            .get(&fingerprint)
            .cloned()
            .ok_or(MutualTlsAuthenticationError::UnmappedCertificate)
    }

    /// Exact authenticated identities present in the certificate map.
    pub fn principals(&self) -> impl Iterator<Item = &AuthenticatedPrincipal> {
        self.principals.values()
    }
}

#[cfg(test)]
mod tests {
    use made_core::value_objects::{AuthenticationMethod, PrincipalKind};

    use super::*;

    fn fingerprint(bytes: &[u8]) -> String {
        format!("{:x}", Sha256::digest(bytes))
    }

    #[test]
    fn exact_certificate_maps_to_mutual_tls_principal() {
        let certificate = b"client certificate der";
        let json = serde_json::json!([{
            "certificate_sha256": fingerprint(certificate),
            "principal_id": "worker-a",
            "principal_kind": "worker"
        }]);
        let map = MutualTlsPrincipalMap::from_json(&serde_json::to_vec(&json).unwrap()).unwrap();

        let principal = map.principal_for_der(certificate).unwrap();
        assert_eq!(principal.id().as_str(), "worker-a");
        assert_eq!(principal.kind(), PrincipalKind::Worker);
        assert_eq!(principal.method(), AuthenticationMethod::MutualTls);
    }

    #[test]
    fn another_valid_certificate_is_not_implicitly_trusted() {
        let json = serde_json::json!([{
            "certificate_sha256": fingerprint(b"mapped"),
            "principal_id": "service-a",
            "principal_kind": "service"
        }]);
        let map = MutualTlsPrincipalMap::from_json(&serde_json::to_vec(&json).unwrap()).unwrap();

        let status = map.principal_for_der(b"other").unwrap_err();
        assert_eq!(status, MutualTlsAuthenticationError::UnmappedCertificate);
    }

    #[test]
    fn malformed_and_duplicate_fingerprints_fail_at_startup() {
        let malformed = serde_json::json!([{
            "certificate_sha256": "not-a-fingerprint",
            "principal_id": "service-a",
            "principal_kind": "service"
        }]);
        assert!(
            MutualTlsPrincipalMap::from_json(&serde_json::to_vec(&malformed).unwrap()).is_err()
        );

        let repeated = fingerprint(b"same");
        let duplicate = serde_json::json!([
            {
                "certificate_sha256": repeated,
                "principal_id": "service-a",
                "principal_kind": "service"
            },
            {
                "certificate_sha256": repeated,
                "principal_id": "service-b",
                "principal_kind": "service"
            }
        ]);
        assert!(
            MutualTlsPrincipalMap::from_json(&serde_json::to_vec(&duplicate).unwrap()).is_err()
        );
    }
}
