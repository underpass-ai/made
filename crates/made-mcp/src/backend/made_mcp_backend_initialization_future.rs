use std::future::Future;
use std::pin::Pin;

/// Async backend startup work performed before the stdio request loop opens.
pub type MadeMcpBackendInitializationFuture<'a> =
    Pin<Box<dyn Future<Output = Result<(), String>> + Send + 'a>>;
