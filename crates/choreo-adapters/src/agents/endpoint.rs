//! Shared validation for provider HTTP endpoints.
//!
//! A provider `endpoint` can be supplied by an untrusted caller through a
//! `RegisterAgent` descriptor attribute (`provider.endpoint`), and the
//! adapter then POSTs proposal/task content — and, for keyed providers, a
//! bearer credential — to it. Validating the endpoint at construction
//! (fail-fast) rejects malformed and non-`http(s)` references with a clear
//! [`DomainError`] instead of surfacing an opaque transport error at the
//! first deliberation.
//!
//! Scheme allowlisting is defence-in-depth: it forbids `file:`, `gopher:`,
//! `data:`, and similar schemes at registration time. It does **not** by
//! itself stop exfiltration to an arbitrary `http(s)` host — restricting
//! *who* may register an agent (authorization) and an operator host
//! allowlist are the complementary layers tracked separately.

use choreo_core::error::DomainError;
use url::Url;

/// Validate and normalise a provider endpoint.
///
/// Returns the trimmed endpoint on success. Errors when it is empty, not a
/// valid absolute URL, or carries a scheme other than `http`/`https`.
pub(crate) fn validate_provider_endpoint(
    field: &'static str,
    raw: impl Into<String>,
) -> Result<String, DomainError> {
    let value = raw.into().trim().to_owned();
    if value.is_empty() {
        return Err(DomainError::EmptyField { field });
    }
    let url = Url::parse(&value).map_err(|_| DomainError::InvariantViolated {
        reason: "provider endpoint must be a valid absolute URL",
    })?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err(DomainError::InvariantViolated {
            reason: "provider endpoint scheme must be http or https",
        });
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_https_and_http() {
        assert_eq!(
            validate_provider_endpoint("t", "https://api.example.com/v1").unwrap(),
            "https://api.example.com/v1"
        );
        assert_eq!(
            validate_provider_endpoint("t", "http://gemma.svc:8000").unwrap(),
            "http://gemma.svc:8000"
        );
    }

    #[test]
    fn trims_surrounding_whitespace() {
        assert_eq!(
            validate_provider_endpoint("t", "  https://x.test  ").unwrap(),
            "https://x.test"
        );
    }

    #[test]
    fn rejects_empty() {
        assert!(matches!(
            validate_provider_endpoint("openai.endpoint", "   ").unwrap_err(),
            DomainError::EmptyField {
                field: "openai.endpoint"
            }
        ));
    }

    #[test]
    fn rejects_non_http_schemes() {
        for bad in [
            "file:///etc/passwd",
            "gopher://evil.test/x",
            "data:text/plain,hi",
            "ftp://host/x",
        ] {
            assert!(
                matches!(
                    validate_provider_endpoint("t", bad).unwrap_err(),
                    DomainError::InvariantViolated { .. }
                ),
                "expected {bad} to be rejected"
            );
        }
    }

    #[test]
    fn rejects_relative_or_garbage() {
        for bad in ["example.com:8000", "not a url", "/v1/chat"] {
            assert!(
                matches!(
                    validate_provider_endpoint("t", bad).unwrap_err(),
                    DomainError::InvariantViolated { .. }
                ),
                "expected {bad} to be rejected"
            );
        }
    }
}
