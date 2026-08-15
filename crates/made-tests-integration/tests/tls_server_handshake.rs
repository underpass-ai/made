//! Epic 8 follow-up: server-mode TLS handshake against the in-process
//! MADE.
//!
//! Mints a self-signed CA + server leaf via `rcgen`, spins up
//! `GrpcFixture::start_with_tls` in `Server` mode, builds a tonic
//! `Channel` that trusts the same CA, and asserts a simple RPC
//! completes — proof that the handshake itself succeeded.

use made_proto::v1::made_service_client::MadeServiceClient;
use made_proto::v1::ListCouncilsRequest;
use made_tests_integration::grpc_fixture::{GrpcFixture, TlsServerSetup};
use made_tests_integration::tls_fixture::mint_tls;
use tonic::transport::{Certificate, ClientTlsConfig, Endpoint};

#[tokio::test]
async fn server_mode_tls_handshake_completes() {
    let tls = mint_tls("localhost");

    let fixture = GrpcFixture::start_with_tls(TlsServerSetup::Server {
        cert: tls.server_cert_pem.clone(),
        key: tls.server_key_pem.clone(),
    })
    .await;

    // Rebuild the channel with the proper CA (not the leaf the fixture
    // currently anchors on for the happy-path). The CA is the actual
    // trust anchor in this fixture's chain.
    let endpoint = Endpoint::from_shared(format!("https://localhost:{}", fixture.addr.port()))
        .unwrap()
        .tls_config(
            ClientTlsConfig::new()
                .ca_certificate(Certificate::from_pem(tls.ca_pem.clone()))
                .domain_name("localhost"),
        )
        .unwrap();
    let channel = endpoint.connect().await.expect("client TLS handshake");

    let mut client = MadeServiceClient::new(channel);
    let response = client
        .list_councils(ListCouncilsRequest {
            include_agents: false,
        })
        .await
        .expect("ListCouncils over server-mode TLS");
    // The fixture starts with no seeded councils — the assertion is
    // that the response arrived, not what it carried.
    let _ = response.into_inner();
}
