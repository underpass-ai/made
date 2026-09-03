use std::fmt;

use made_core::error::DomainError;

/// Client certificate + private key (PEM-encoded) for mTLS-protected
/// vLLM endpoints. The bytes are held in memory and fed to
/// [`reqwest::Identity`] when the HTTP client is built; they never
/// appear in `Debug` output.
#[derive(Clone)]
pub struct VllmClientIdentity {
    pem_bundle: Vec<u8>,
}

impl VllmClientIdentity {
    /// Build an identity from concatenated cert + key PEM. The two
    /// inputs are joined with a newline so the PEM separators stay
    /// well-formed even if the caller forgot the trailing `\n`.
    pub fn from_cert_and_key(cert_pem: &[u8], key_pem: &[u8]) -> Result<Self, DomainError> {
        if cert_pem.is_empty() {
            return Err(DomainError::EmptyField {
                field: "vllm.client_cert_pem",
            });
        }
        if key_pem.is_empty() {
            return Err(DomainError::EmptyField {
                field: "vllm.client_key_pem",
            });
        }
        let mut bundle = Vec::with_capacity(cert_pem.len() + key_pem.len() + 1);
        bundle.extend_from_slice(cert_pem);
        if !cert_pem.ends_with(b"\n") {
            bundle.push(b'\n');
        }
        bundle.extend_from_slice(key_pem);
        Ok(Self { pem_bundle: bundle })
    }

    pub(super) fn expose(&self) -> &[u8] {
        &self.pem_bundle
    }
}

impl fmt::Debug for VllmClientIdentity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("VllmClientIdentity(**redacted**)")
    }
}
