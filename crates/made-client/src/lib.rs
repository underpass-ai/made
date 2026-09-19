//! Reusable client for MADE's public gRPC surface.

mod actions;
mod artifact_export;
mod artifacts;
mod budgets;
mod ceremonies;
mod ceremony_tree;
mod ceremony_tree_node;
mod client_config;
mod made_client;
mod made_client_error;
mod progress;
mod progress_batch;
mod progress_checkpoint;
mod reports;

pub use ceremony_tree::CeremonyTree;
pub use ceremony_tree_node::CeremonyTreeNode;
pub use client_config::ClientConfig;
pub use made_client::MadeClient;
pub use made_client_error::MadeClientError;
pub use progress_batch::ProgressBatch;
pub use progress_checkpoint::ProgressCheckpoint;

pub use made_proto::v1;

#[cfg(test)]
mod tests;
