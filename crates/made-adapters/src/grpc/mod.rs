//! gRPC server adapter.
//!
//! Implements the `MadeService` trait generated from the
//! `underpass.made.v1` proto. All RPC handlers are thin: they map
//! the incoming proto message to a domain input, delegate to a use
//! case in `made-app`, and map the result (or the [`DomainError`])
//! back onto a proto response or [`tonic::Status`].
//!
//! Nothing in this module adds behaviour; it is a pure transport
//! translation. Use-case or provider-specific vocabulary must never
//! leak into the server — all vocabulary belongs to the proto
//! contract.
//!
//! [`DomainError`]: made_core::error::DomainError

mod made_grpc_service_builder;
mod mappers;
mod mutual_tls_authentication_error;
mod mutual_tls_principal_map;
mod mutual_tls_principal_mapping_entry;
mod service;
mod status;
mod stream;
pub(crate) mod tracecontext;

pub use made_grpc_service_builder::MadeGrpcServiceBuilder;
pub use mutual_tls_authentication_error::MutualTlsAuthenticationError;
pub use mutual_tls_principal_map::MutualTlsPrincipalMap;
pub use mutual_tls_principal_mapping_entry::MutualTlsPrincipalMappingEntry;
pub use service::MadeGrpcService;
pub use status::{budget_error_to_status, domain_error_to_status};
