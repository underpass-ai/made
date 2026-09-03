use std::future::Future;
use std::pin::Pin;

use serde_json::Value;

/// Async tool-call future shape. Boxed so the trait stays object-safe.
pub type MadeMcpToolFuture<'a> = Pin<Box<dyn Future<Output = Result<Value, String>> + Send + 'a>>;
