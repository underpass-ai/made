//! The protected durable embedded composition the plugin launcher runs.

use std::sync::Arc;

use super::{embedded_step_continuation, MadeMcpServer};
use crate::backend::EVENT_SINK_PATH_ENV;
use crate::embedded::EmbeddedMadeMcpBackend;
use crate::human_approval_source::HumanApprovalSource;

impl MadeMcpServer {
    /// The protected durable embedded engine: one SQLite file, the
    /// authorization policy it carries, and the trusted host this
    /// process acts as. This is what the plugin launcher composes.
    ///
    /// # Errors
    ///
    /// Returns the store's or the policy's failure to open, or a
    /// misspelt [`HUMAN_APPROVAL_SOURCE_ENV`](crate::human_approval_source::HUMAN_APPROVAL_SOURCE_ENV).
    pub fn embedded_sqlite_authorized(
        path: impl AsRef<std::path::Path>,
        policy_id: &str,
        trusted_host_id: &str,
    ) -> Result<Self, String> {
        use made_adapters::artifacts::LocalArtifactStore;
        use made_adapters::clock::SystemClock;
        use made_adapters::sqlite::SqliteAuthorizationPolicyStore;
        use made_app::authorization::{
            AuthorizeOperationUseCase, ReadAuthorizationPolicyUseCase, TrustedHostAuthorizationGate,
        };
        use made_core::ports::{
            ArtifactStorePort, AuthorizationPolicyStorePort, ExecutionReceiptStorePort,
        };
        use made_core::value_objects::{
            AuthenticatedPrincipal, AuthenticationMethod, AuthorizationDecisionTtl,
            AuthorizationPolicyId, PrincipalId, PrincipalKind,
        };

        let human_approval_source = HumanApprovalSource::from_env()?;
        let path = path.as_ref();
        let metrics = Arc::new(
            made_adapters::metrics::PrometheusMetricsRecorder::new()
                .map_err(|error| format!("failed to initialize embedded metrics: {error}"))?,
        );
        let sink = std::env::var(EVENT_SINK_PATH_ENV)
            .ok()
            .filter(|value| !value.trim().is_empty())
            .map(|sink_path| {
                made_adapters::event_sink::JsonLinesCeremonyEventSink::open_with_metrics(
                    sink_path,
                    metrics.clone(),
                )
            })
            .transpose()
            .map_err(|error| format!("failed to open {EVENT_SINK_PATH_ENV}: {error}"))?;
        let transport = sink
            .map(|sink| Arc::new(sink) as Arc<dyn made_core::ports::CeremonyEventTransportPort>);
        let activation = made_adapters::activation::select_host_activation()
            .map_err(|error| format!("host activation is misconfigured: {error}"))?;
        let made = made_embedded::EmbeddedMade::open_with_host_activation(
            path, metrics, transport, activation,
        )
        .map_err(|error| {
            format!(
                "failed to open the embedded SQLite ceremony store at `{}`: {error}",
                path.display()
            )
        })?;
        let store: Arc<dyn AuthorizationPolicyStorePort> =
            Arc::new(SqliteAuthorizationPolicyStore::open(path).map_err(|error| {
                format!("failed to open embedded authorization store: {error}")
            })?);
        let policy_id = AuthorizationPolicyId::new(policy_id).map_err(|error| error.to_string())?;
        let principal = AuthenticatedPrincipal::new(
            PrincipalId::new(trusted_host_id).map_err(|error| error.to_string())?,
            PrincipalKind::TrustedHost,
            AuthenticationMethod::LocalHostPolicy,
        )
        .map_err(|error| error.to_string())?;
        let clock = Arc::new(SystemClock::new());
        let authorize = Arc::new(AuthorizeOperationUseCase::new(
            policy_id.clone(),
            store.clone(),
            clock.clone(),
            AuthorizationDecisionTtl::from_seconds(60).expect("fixed TTL is valid"),
        ));
        let gate = TrustedHostAuthorizationGate::new(authorize, principal)
            .map_err(|error| error.to_string())?;
        let read_policy = ReadAuthorizationPolicyUseCase::new(policy_id.clone(), store.clone());
        let (step_continuation, ceremony_store) =
            embedded_step_continuation::wire(path, policy_id.clone(), store.clone(), clock)?;
        let made = made.with_authorization_policy(policy_id, store);
        let receipts: Arc<dyn ExecutionReceiptStorePort> = ceremony_store;
        let mut artifact_root = path.as_os_str().to_owned();
        artifact_root.push(".artifacts");
        let artifacts: Arc<dyn ArtifactStorePort> = Arc::new(
            LocalArtifactStore::open(std::path::PathBuf::from(artifact_root)).map_err(|error| {
                format!("failed to open artifact authorization resolver: {error}")
            })?,
        );
        Ok(Self::with_backend(
            EmbeddedMadeMcpBackend::with_authorization(
                made,
                gate,
                read_policy,
                step_continuation,
                artifacts,
                receipts,
            )
            .with_human_approval_source(human_approval_source),
        ))
    }
}
