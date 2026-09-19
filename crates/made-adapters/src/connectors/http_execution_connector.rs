use super::HttpOperationResponse;
use crate::execution::operation_record::{invalid, key, seal};
use async_trait::async_trait;
use made_core::value_objects::{
    ArtifactSourceKind, ExecutionConnectorId, ExecutionIntent, ExecutionRecoveryCapability,
};
use made_core::{
    ports::{
        CeremonyExecutionConnectorOutcome as Outcome, CeremonyExecutionConnectorPort,
        CeremonyExecutionRequest, ExecutionCancellation,
    },
    DomainError,
};
use reqwest::{Client, StatusCode, Url};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// HTTP reference protocol: PUT /operations/{stable-key}, GET at the same URL.
/// A lost or unqueryable effect is reconciled explicitly, never blindly replayed.
#[derive(Debug)]
pub struct HttpExecutionConnector {
    id: ExecutionConnectorId,
    base: Url,
    client: Client,
    operation_root: PathBuf,
}

impl HttpExecutionConnector {
    pub fn new(
        id: ExecutionConnectorId,
        base: &str,
        operation_root: impl AsRef<Path>,
        timeout: Duration,
    ) -> Result<Self, DomainError> {
        let mut base = Url::parse(base).map_err(|_| invalid("HTTP base URL invalid"))?;
        let local = base.host_str().is_some_and(|host| {
            host == "localhost"
                || host
                    .parse::<std::net::IpAddr>()
                    .is_ok_and(|ip| ip.is_loopback())
        });
        if timeout.is_zero()
            || (base.scheme() != "https" && !(base.scheme() == "http" && local))
            || !base.username().is_empty()
            || base.password().is_some()
            || base.query().is_some()
            || base.fragment().is_some()
        {
            return Err(invalid(
                "HTTP endpoint requires HTTPS or explicit loopback HTTP",
            ));
        }
        if !base.path().ends_with('/') {
            base.set_path(&format!("{}/", base.path()));
        }
        std::fs::create_dir_all(operation_root.as_ref())
            .map_err(|_| invalid("HTTP operation root unavailable"))?;
        let operation_root = operation_root
            .as_ref()
            .canonicalize()
            .map_err(|_| invalid("HTTP operation root unavailable"))?;
        let client = Client::builder()
            .timeout(timeout)
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|_| invalid("HTTP client unavailable"))?;
        Ok(Self {
            id,
            base,
            client,
            operation_root,
        })
    }
    fn validate(&self, intent: &ExecutionIntent) -> Result<Url, DomainError> {
        intent.validate()?;
        if intent.connector_id() != &self.id
            || intent.recovery_capability() != self.recovery_capability()
        {
            return Err(invalid("HTTP connector identity mismatch"));
        }
        self.base
            .join(&format!("operations/{}", key(intent)))
            .map_err(|_| invalid("HTTP operation URL invalid"))
    }
    fn unresolved(intent: &ExecutionIntent) -> Outcome {
        Outcome::ReconciliationRequired(intent.operation().operation_id().clone())
    }
    async fn response(
        mut response: reqwest::Response,
        intent: &ExecutionIntent,
    ) -> Result<Outcome, DomainError> {
        if !response.status().is_success() {
            return Ok(Self::unresolved(intent));
        }
        let mut body = Vec::new();
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|_| invalid("HTTP operation response interrupted"))?
        {
            if body.len().saturating_add(chunk.len()) > 65536 {
                return Err(invalid("HTTP operation response exceeded limit"));
            }
            body.extend_from_slice(&chunk);
        }
        let result: HttpOperationResponse = serde_json::from_slice(&body)
            .map_err(|_| invalid("HTTP operation response invalid"))?;
        Ok(Outcome::Observed(Box::new(result.observe(intent)?)))
    }
    async fn query(&self, intent: &ExecutionIntent) -> Result<Outcome, DomainError> {
        let url = self.validate(intent)?;
        match self.client.get(url).send().await {
            Ok(response) => match Self::response(response, intent).await {
                Ok(outcome) => Ok(outcome),
                Err(DomainError::Conflict { what }) => Err(DomainError::Conflict { what }),
                Err(_) => Ok(Self::unresolved(intent)),
            },
            Err(_) => Ok(Self::unresolved(intent)),
        }
    }
    async fn run(
        &self,
        intent: &ExecutionIntent,
        cancellation: ExecutionCancellation,
    ) -> Result<Outcome, DomainError> {
        let url = self.validate(intent)?;
        if cancellation.is_cancelled() {
            return Err(invalid("execution authority was cancelled"));
        }
        // Only an authoritative 404 on the first attempt permits admission.
        let Ok(query) = self.client.get(url.clone()).send().await else {
            return Ok(Self::unresolved(intent));
        };
        if query.status() != StatusCode::NOT_FOUND {
            return Self::response(query, intent)
                .await
                .or_else(|error| match error {
                    DomainError::Conflict { .. } => Err(error),
                    _ => Ok(Self::unresolved(intent)),
                });
        }
        if !seal(
            &self
                .operation_root
                .join(format!("{}.admitted", key(intent))),
            intent.operation().request_digest().as_str().as_bytes(),
        )? {
            return Ok(Self::unresolved(intent));
        }
        if cancellation.is_cancelled() {
            return Ok(Self::unresolved(intent));
        }
        let request: serde_json::Value =
            serde_json::from_slice(intent.operation().request().as_bytes())
                .map_err(|_| invalid("HTTP semantic request invalid"))?;
        let body = serde_json::json!({"operation_id":intent.operation().operation_id(),"request_digest":intent.operation().request_digest(),"producer_claim_fence":intent.claim_fence(),"request":request});
        let sent = cancellation
            .run(
                self.client
                    .put(url)
                    .header(
                        "Idempotency-Key",
                        intent.operation().operation_id().as_str(),
                    )
                    .header(
                        "X-Request-Digest",
                        intent.operation().request_digest().as_str(),
                    )
                    .json(&body)
                    .send(),
            )
            .await;
        if cancellation.is_cancelled() {
            return Ok(Self::unresolved(intent));
        }
        if let Some(Ok(response)) = sent {
            match Self::response(response, intent).await {
                Ok(observed @ Outcome::Observed(_)) => return Ok(observed),
                Err(error @ DomainError::Conflict { .. }) => return Err(error),
                _ => {}
            }
        }
        self.query(intent).await
    }
}

#[async_trait]
impl CeremonyExecutionConnectorPort for HttpExecutionConnector {
    fn connector_id(&self) -> &ExecutionConnectorId {
        &self.id
    }
    fn recovery_capability(&self) -> ExecutionRecoveryCapability {
        ExecutionRecoveryCapability::QueryableByOperationId
    }
    fn source_kind(&self) -> ArtifactSourceKind {
        ArtifactSourceKind::ExternalExecution
    }
    async fn execute_or_recover(
        &self,
        request: CeremonyExecutionRequest,
    ) -> Result<Outcome, DomainError> {
        self.run(request.intent(), ExecutionCancellation::new())
            .await
    }
    async fn execute_cancellable(
        &self,
        request: CeremonyExecutionRequest,
        cancellation: ExecutionCancellation,
    ) -> Result<Outcome, DomainError> {
        self.run(request.intent(), cancellation).await
    }
    async fn recover_intent(&self, intent: &ExecutionIntent) -> Result<Outcome, DomainError> {
        self.query(intent).await
    }
}
