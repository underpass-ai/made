use clap::Parser;
use std::path::PathBuf;

use crate::{Command, OutputFormat};

/// Operate MADE through its public gRPC API.
#[derive(Debug, Parser)]
#[command(version, about)]
pub struct Args {
    /// Public MADE gRPC endpoint.
    #[arg(long, env = "MADE_ENDPOINT", default_value = "http://127.0.0.1:50055")]
    pub endpoint: String,
    /// Stable namespace used to derive one x-made-request-id per RPC payload.
    #[arg(long, env = "MADE_REQUEST_NAMESPACE")]
    pub request_namespace: Option<String>,
    /// Additional PEM CA certificate for the MADE endpoint.
    #[arg(long, env = "MADE_TLS_CA_CERTIFICATE")]
    pub tls_ca_certificate: Option<PathBuf>,
    /// PEM client certificate chain used for mTLS.
    #[arg(long, env = "MADE_TLS_CLIENT_CERTIFICATE", requires = "tls_client_key")]
    pub tls_client_certificate: Option<PathBuf>,
    /// PEM private key used for mTLS.
    #[arg(long, env = "MADE_TLS_CLIENT_KEY", requires = "tls_client_certificate")]
    pub tls_client_key: Option<PathBuf>,
    /// TLS server name when it differs from the endpoint host.
    #[arg(long, env = "MADE_TLS_DOMAIN_NAME")]
    pub tls_domain_name: Option<String>,
    /// Output format for metadata and events.
    #[arg(long, value_enum, default_value_t = OutputFormat::Text)]
    pub output: OutputFormat,
    #[command(subcommand)]
    pub command: Command,
}
