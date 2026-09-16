use std::future::Future;
use std::pin::Pin;

use serde_json::Value;

use crate::protocol::ToolError;

/// Async tool-call future shape. Boxed so the trait stays object-safe.
///
/// The error side is the one envelope both backends speak, not a
/// string: a caller branches on `ToolError::code` instead of reading
/// whichever prose the backend that answered happened to produce.
pub type MadeMcpToolFuture<'a> =
    Pin<Box<dyn Future<Output = Result<Value, ToolError>> + Send + 'a>>;
