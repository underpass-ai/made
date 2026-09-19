//! Typed contracts for composing durable execution connectors.
//!
//! This module deliberately stops at the adapter boundary.  A repository
//! script owns the external effect, a filesystem connector owns durable
//! receipt storage, and a transport connector owns publication.  The
//! orchestrator only wires those responsibilities together and never puts
//! request bytes, command arguments, or backend error text in a receipt.

mod contract;
mod durable_execution;
mod durable_execution_result;
mod error;
mod filesystem;
mod protocol;

pub use contract::{
    ConnectorCapabilities, ConnectorCapability, ConnectorContractError, ConnectorDescriptor,
    ConnectorKind,
};
pub use durable_execution::DurableExecutionConnector;
pub use durable_execution_result::DurableExecutionResult;
pub use error::ConnectorError;
pub use filesystem::JsonFileReceiptStore;
pub use protocol::{
    ReceiptFilesystemConnector, ReceiptStatus, ReceiptTransportConnector,
    RepositoryScriptConnector, SafeExecutionReceipt, ScriptInvocation, ScriptObservation,
    ScriptResolution,
};

#[cfg(test)]
mod tests;

mod git_execution_connector;
mod git_execution_request;
pub use git_execution_connector::GitExecutionConnector;
pub use git_execution_request::GitExecutionRequest;

#[cfg(feature = "_http")]
mod http_execution_connector;
#[cfg(feature = "_http")]
mod http_operation_response;
#[cfg(feature = "_http")]
pub use http_execution_connector::HttpExecutionConnector;
#[cfg(feature = "_http")]
pub use http_operation_response::HttpOperationResponse;
