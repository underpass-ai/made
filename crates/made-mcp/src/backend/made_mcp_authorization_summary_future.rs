use std::future::Future;
use std::pin::Pin;

use serde_json::Value;

/// What the principal a backend acts as may do, read from its policy.
///
/// `None` when the backend acts as no authenticated principal of its
/// own — the fixture, the gRPC client (the service decides there) and
/// an unprotected in-memory engine. `Some(Err)` when it does but the
/// policy could not be read: discovery says so rather than pretending
/// the host holds nothing.
pub type MadeMcpAuthorizationSummaryFuture<'a> =
    Pin<Box<dyn Future<Output = Option<Result<Value, String>>> + Send + 'a>>;
