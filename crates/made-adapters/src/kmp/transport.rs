//! The pipe to a memory kernel, and what can go wrong on it.

use async_trait::async_trait;
use serde_json::Value;

use super::{KernelAnswer, KernelTransportError};

/// Calling one tool on a memory kernel.
///
/// One method, because that is the whole protocol: everything the
/// kernel offers is a named tool taking a JSON document. Keeping the
/// trait this narrow is what lets a test stand in for a kernel
/// without standing in for a process.
#[async_trait]
pub trait KernelTransport: Send + Sync {
    async fn call(
        &self,
        tool: &str,
        arguments: Value,
    ) -> Result<KernelAnswer, KernelTransportError>;
}
