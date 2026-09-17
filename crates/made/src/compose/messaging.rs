//! The messaging phase of the composition root.
//!
//! Whether the application talks over NATS or to nobody is one
//! decision with one set of consequences: the port the use cases
//! publish through, the inbound subscriber that cannot be built until
//! the dispatch service exists, and the client handle the readiness
//! probe needs. They are wired here, together, so `compose` takes the
//! decision once and reads back three settled values.

use std::sync::Arc;

use made_adapters::config::ServiceConfig;
use made_adapters::nats::{
    NatsCeremonyEventTransport, NatsConfig, NatsMessaging, NatsTriggerSubscriber,
};
use made_adapters::noop::NoopMessaging;
use made_app::services::AutoDispatchService;
use made_core::ports::{CeremonyEventTransportPort, MessagingPort, MetricsRecorderPort};
use tracing::info;

use crate::ComposeError;

/// Factory closure that produces a [`NatsTriggerSubscriber`] once the
/// application's `AutoDispatchService` has been constructed.
type SubscriberFactory = Box<dyn FnOnce(Arc<AutoDispatchService>) -> NatsTriggerSubscriber>;

/// How long to wait for NATS to be reachable during startup.
///
/// Deployments bring NATS and MADE up together (compose,
/// Kubernetes, etc.). Failing fast on the first connection attempt
/// means any transient unavailability forces a restart; a bounded
/// retry is the production-correct behaviour.
const NATS_CONNECT_BUDGET: std::time::Duration = std::time::Duration::from_secs(30);

async fn connect_nats_with_retry(
    url: &str,
    total_budget: std::time::Duration,
) -> Result<async_nats::Client, ComposeError> {
    let deadline = std::time::Instant::now() + total_budget;
    let mut last_err: Option<async_nats::ConnectError> = None;
    while std::time::Instant::now() < deadline {
        match async_nats::connect(url).await {
            Ok(client) => return Ok(client),
            Err(err) => {
                tracing::warn!(url, error = %err, "nats not ready yet; retrying");
                last_err = Some(err);
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            }
        }
    }
    Err(ComposeError::NatsConnect(last_err.unwrap_or_else(|| {
        // Unreachable: the loop exits only after at least one attempt.
        panic!("nats connect budget elapsed with no error recorded")
    })))
}

/// Everything `compose` needs out of the messaging wiring phase:
/// the port implementation that use cases talk through, a factory
/// for the inbound subscriber, and — when NATS is wired — a handle
/// to the live client so the health endpoints can probe it.
pub(super) struct MessagingWiring {
    pub(super) port: Arc<dyn MessagingPort>,
    pub(super) subscriber_factory: Option<SubscriberFactory>,
    pub(super) nats_client: Option<async_nats::Client>,
    pub(super) ceremony_transport: Option<Arc<dyn CeremonyEventTransportPort>>,
}

pub(super) async fn wire_messaging(
    cfg: &ServiceConfig,
    metrics: Arc<dyn MetricsRecorderPort>,
) -> Result<MessagingWiring, ComposeError> {
    if !cfg.nats_enabled {
        info!("nats disabled; using noop messaging");
        let port: Arc<dyn MessagingPort> = Arc::new(NoopMessaging::new());
        return Ok(MessagingWiring {
            port,
            subscriber_factory: None,
            nats_client: None,
            ceremony_transport: None,
        });
    }

    let nats_cfg = NatsConfig::new(&cfg.nats_url, &cfg.publish_prefix, &cfg.trigger_subject)?;
    let client = connect_nats_with_retry(&nats_cfg.url, NATS_CONNECT_BUDGET).await?;
    info!(url = nats_cfg.url.as_str(), "nats connected");

    let port: Arc<dyn MessagingPort> = Arc::new(
        NatsMessaging::new(client.clone(), nats_cfg.subjects.clone()).with_metrics(metrics),
    );

    let subjects = nats_cfg.subjects.clone();
    let ceremony_transport = Arc::new(NatsCeremonyEventTransport::new(
        client.clone(),
        subjects.clone(),
    ));
    let factory_client = client.clone();
    let subscriber_factory: SubscriberFactory =
        Box::new(move |dispatch| NatsTriggerSubscriber::new(factory_client, subjects, dispatch));

    Ok(MessagingWiring {
        port,
        subscriber_factory: Some(subscriber_factory),
        nats_client: Some(client),
        ceremony_transport: Some(ceremony_transport),
    })
}
