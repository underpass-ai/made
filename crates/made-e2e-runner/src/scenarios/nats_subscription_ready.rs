use std::time::Duration;

use anyhow::{ensure, Context, Result};
use async_nats::Client;
use futures::StreamExt;

const MARKER: &[u8] = b"made-e2e-ready";

/// Confirm the server has processed earlier SUB commands on this client.
///
/// async-nats flush only drains local writes. Receiving a marker on a private
/// inbox establishes a server round trip before another connection triggers
/// the business event. The marker never enters a business subject.
pub(super) async fn wait_for_subscription_ready(client: &Client) -> Result<()> {
    tokio::time::timeout(Duration::from_secs(5), async {
        let inbox = client.new_inbox();
        let mut barrier = client
            .subscribe(inbox.clone())
            .await
            .context("subscribe NATS readiness inbox")?;
        client
            .publish(inbox, MARKER.into())
            .await
            .context("publish NATS readiness marker")?;
        let message = barrier
            .next()
            .await
            .context("NATS readiness inbox closed before receiving marker")?;
        ensure!(
            message.payload.as_ref() == MARKER,
            "unexpected NATS readiness marker"
        );
        Ok(())
    })
    .await
    .context("NATS subscription readiness round trip timed out")?
}

#[cfg(test)]
mod tests;
