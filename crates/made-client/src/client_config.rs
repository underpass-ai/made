use std::time::Duration;

/// Bounded connection policy for the public MADE endpoint.
#[derive(Clone, Debug)]
pub struct ClientConfig {
    endpoint: String,
    request_id: String,
    connect_attempts: u32,
    initial_backoff: Duration,
    maximum_backoff: Duration,
    tls_domain_name: Option<String>,
    ca_certificate_pem: Option<Vec<u8>>,
    client_identity_pem: Option<(Vec<u8>, Vec<u8>)>,
}

impl ClientConfig {
    pub fn new(endpoint: impl Into<String>) -> Self {
        Self {
            endpoint: endpoint.into(),
            request_id: uuid::Uuid::new_v4().to_string(),
            connect_attempts: 5,
            initial_backoff: Duration::from_millis(100),
            maximum_backoff: Duration::from_secs(2),
            tls_domain_name: None,
            ca_certificate_pem: None,
            client_identity_pem: None,
        }
    }

    /// Reuse one caller-generated id across every RPC and reconnect in this client.
    #[must_use]
    pub fn with_request_id(mut self, request_id: impl Into<String>) -> Self {
        self.request_id = request_id.into();
        self
    }

    /// Trust an additional PEM-encoded CA for the public endpoint.
    #[must_use]
    pub fn with_ca_certificate_pem(mut self, pem: impl Into<Vec<u8>>) -> Self {
        self.ca_certificate_pem = Some(pem.into());
        self
    }

    /// Present a PEM certificate chain and private key as the mTLS client identity.
    #[must_use]
    pub fn with_mtls_identity_pem(
        mut self,
        certificate_pem: impl Into<Vec<u8>>,
        private_key_pem: impl Into<Vec<u8>>,
    ) -> Self {
        self.client_identity_pem = Some((certificate_pem.into(), private_key_pem.into()));
        self
    }

    /// Override the TLS server name when it differs from the endpoint host.
    #[must_use]
    pub fn with_tls_domain_name(mut self, domain_name: impl Into<String>) -> Self {
        self.tls_domain_name = Some(domain_name.into());
        self
    }

    #[must_use]
    pub fn with_connect_attempts(mut self, attempts: u32) -> Self {
        self.connect_attempts = attempts.max(1);
        self
    }

    #[must_use]
    pub fn with_backoff(mut self, initial: Duration, maximum: Duration) -> Self {
        self.initial_backoff = initial;
        self.maximum_backoff = maximum.max(initial);
        self
    }

    pub(crate) fn endpoint(&self) -> &str {
        &self.endpoint
    }

    pub(crate) fn request_id(&self) -> &str {
        &self.request_id
    }

    pub(crate) fn connect_attempts(&self) -> u32 {
        self.connect_attempts
    }

    pub(crate) fn initial_backoff(&self) -> Duration {
        self.initial_backoff
    }

    pub(crate) fn maximum_backoff(&self) -> Duration {
        self.maximum_backoff
    }

    pub(crate) fn uses_tls(&self) -> bool {
        self.endpoint.starts_with("https://")
            || self.ca_certificate_pem.is_some()
            || self.client_identity_pem.is_some()
            || self.tls_domain_name.is_some()
    }

    pub(crate) fn tls_domain_name(&self) -> Option<&str> {
        self.tls_domain_name.as_deref()
    }

    pub(crate) fn ca_certificate_pem(&self) -> Option<&[u8]> {
        self.ca_certificate_pem.as_deref()
    }

    pub(crate) fn client_identity_pem(&self) -> Option<(&[u8], &[u8])> {
        self.client_identity_pem
            .as_ref()
            .map(|(certificate, key)| (certificate.as_slice(), key.as_slice()))
    }
}
