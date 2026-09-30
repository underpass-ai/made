//! Process shutdown signals for the daemon lifecycle.

use tokio::signal::unix::{signal, Signal, SignalKind};
use tracing::{error, info};

/// SIGTERM and SIGINT streams, registered with the OS as soon as they are
/// installed so that neither signal can fall back to its default action.
pub(super) struct ShutdownSignals {
    terminate: Option<Signal>,
    interrupt: Option<Signal>,
}

impl ShutdownSignals {
    pub(super) fn install() -> Self {
        let terminate = signal(SignalKind::terminate())
            .inspect_err(|err| {
                error!(error = %err, "cannot install SIGTERM handler; shutdown relies on SIGINT only");
            })
            .ok();
        let interrupt = signal(SignalKind::interrupt())
            .inspect_err(|err| error!(error = %err, "cannot install SIGINT handler"))
            .ok();
        Self {
            terminate,
            interrupt,
        }
    }

    pub(super) async fn recv(mut self) {
        async fn next(signal: Option<&mut Signal>) {
            match signal {
                Some(signal) => {
                    signal.recv().await;
                }
                None => std::future::pending().await,
            }
        }
        tokio::select! {
            () = next(self.terminate.as_mut()) => info!("received SIGTERM; shutting down"),
            () = next(self.interrupt.as_mut()) => info!("received SIGINT; shutting down"),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::process::Command;
    use std::time::Duration;

    use super::ShutdownSignals;

    #[tokio::test]
    async fn sigterm_right_after_install_is_received_not_fatal() {
        // No task has polled anything yet: the handler must already be armed,
        // otherwise this SIGTERM takes the default action and kills the test.
        let signals = ShutdownSignals::install();
        let sent = Command::new("kill")
            .args(["-TERM", &std::process::id().to_string()])
            .status()
            .unwrap();
        assert!(sent.success());
        tokio::time::timeout(Duration::from_secs(10), signals.recv())
            .await
            .expect("an armed SIGTERM handler resolves recv");
    }
}
