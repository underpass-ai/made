//! Reusable client for MADE's public gRPC surface.

mod actions;
mod agent_progress;
mod agent_progress_batch;
mod agent_progress_filter;
mod artifact_export;
mod artifacts;
mod authorization;
mod budgets;
mod ceremonies;
mod ceremony_search_page;
mod ceremony_tree;
mod ceremony_tree_node;
mod client_config;
mod execution_receipts;
mod made_client;
mod made_client_error;
mod progress;
mod progress_batch;
mod progress_checkpoint;
mod reports;
mod request_context;

pub use agent_progress_batch::AgentProgressBatch;
pub use agent_progress_filter::AgentProgressFilter;
pub use authorization::authorization_target_digest;
pub use ceremony_search_page::CeremonySearchPage;
pub use ceremony_tree::CeremonyTree;
pub use ceremony_tree_node::CeremonyTreeNode;
pub use client_config::ClientConfig;
pub use made_client::MadeClient;
pub use made_client_error::MadeClientError;
pub use progress_batch::ProgressBatch;
pub use progress_checkpoint::ProgressCheckpoint;
pub use request_context::RequestContext;

pub use made_proto::v1;

#[cfg(test)]
mod request_id_tests;
#[cfg(test)]
mod tests;
