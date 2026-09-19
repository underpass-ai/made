#![cfg(unix)]

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use made_proto::made_service_client::MadeServiceClient;
use made_proto::GetStatusRequest;
use rcgen::{CertificateParams, DistinguishedName, DnType, IsCa, KeyUsagePurpose};
use sha2::{Digest, Sha256};
use tonic::transport::{Certificate, ClientTlsConfig, Endpoint, Identity};
use tonic::{Code, Request};

struct ServerProcess(Child);

struct TlsMaterial {
    ca_pem: Vec<u8>,
    server_cert_pem: Vec<u8>,
    server_key_pem: Vec<u8>,
    client_cert_pem: Vec<u8>,
    client_key_pem: Vec<u8>,
    client_der: Vec<u8>,
}

struct ServerConfiguration {
    ca: std::path::PathBuf,
    server_certificate: std::path::PathBuf,
    server_key: std::path::PathBuf,
    principals: std::path::PathBuf,
    store: std::path::PathBuf,
}

impl Drop for ServerProcess {
    fn drop(&mut self) {
        if self.0.try_wait().ok().flatten().is_none() {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
}

fn http_get(port: u16, path: &str) -> std::io::Result<String> {
    let mut stream = TcpStream::connect_timeout(
        &format!("127.0.0.1:{port}").parse().unwrap(),
        Duration::from_millis(100),
    )?;
    stream.set_read_timeout(Some(Duration::from_secs(2)))?;
    write!(
        stream,
        "GET {path} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n"
    )?;
    let mut body = String::new();
    stream.read_to_string(&mut body)?;
    Ok(body)
}

fn mint_tls() -> TlsMaterial {
    let mut ca_params = CertificateParams::new(Vec::<String>::new()).unwrap();
    ca_params.is_ca = IsCa::Ca(rcgen::BasicConstraints::Unconstrained);
    ca_params.key_usages.push(KeyUsagePurpose::KeyCertSign);
    let ca_key = rcgen::KeyPair::generate().unwrap();
    let ca_cert = ca_params.self_signed(&ca_key).unwrap();

    let mut server_params = CertificateParams::new(vec!["localhost".to_owned()]).unwrap();
    let mut server_name = DistinguishedName::new();
    server_name.push(DnType::CommonName, "localhost");
    server_params.distinguished_name = server_name;
    let server_key = rcgen::KeyPair::generate().unwrap();
    let server_cert = server_params
        .signed_by(&server_key, &ca_cert, &ca_key)
        .unwrap();

    let mut client_params = CertificateParams::new(Vec::<String>::new()).unwrap();
    let mut client_name = DistinguishedName::new();
    client_name.push(DnType::CommonName, "lifecycle-client");
    client_params.distinguished_name = client_name;
    let client_key = rcgen::KeyPair::generate().unwrap();
    let client_cert = client_params
        .signed_by(&client_key, &ca_cert, &ca_key)
        .unwrap();

    TlsMaterial {
        ca_pem: ca_cert.pem().into_bytes(),
        server_cert_pem: server_cert.pem().into_bytes(),
        server_key_pem: server_key.serialize_pem().into_bytes(),
        client_cert_pem: client_cert.pem().into_bytes(),
        client_key_pem: client_key.serialize_pem().into_bytes(),
        client_der: client_cert.der().to_vec(),
    }
}

fn configure_authorization(directory: &std::path::Path, tls: &TlsMaterial) -> ServerConfiguration {
    let configuration = ServerConfiguration {
        ca: directory.join("ca.pem"),
        server_certificate: directory.join("server-cert.pem"),
        server_key: directory.join("server-key.pem"),
        principals: directory.join("principals.json"),
        store: directory.join("made.sqlite3"),
    };
    std::fs::write(&configuration.ca, &tls.ca_pem).unwrap();
    std::fs::write(&configuration.server_certificate, &tls.server_cert_pem).unwrap();
    std::fs::write(&configuration.server_key, &tls.server_key_pem).unwrap();
    std::fs::write(
        &configuration.principals,
        serde_json::to_vec(&serde_json::json!([{
            "certificate_sha256": format!("{:x}", Sha256::digest(&tls.client_der)),
            "principal_id": "lifecycle-owner",
            "principal_kind": "trusted_host"
        }]))
        .unwrap(),
    )
    .unwrap();
    let bootstrap = Command::new(env!("CARGO_BIN_EXE_made"))
        .args([
            "bootstrap-authorization",
            "--policy-id",
            "lifecycle-policy",
            "--trusted-host-id",
            "lifecycle-owner",
        ])
        .env("MADE_CEREMONY_STORE_PATH", &configuration.store)
        .output()
        .unwrap();
    assert!(
        bootstrap.status.success(),
        "bootstrap failed: {}",
        String::from_utf8_lossy(&bootstrap.stderr)
    );
    configuration
}

fn spawn_server(configuration: &ServerConfiguration) -> (ServerProcess, u16, u16) {
    let grpc_reservation = TcpListener::bind("127.0.0.1:0").unwrap();
    let http_reservation = TcpListener::bind("127.0.0.1:0").unwrap();
    let grpc_port = grpc_reservation.local_addr().unwrap().port();
    let http_port = http_reservation.local_addr().unwrap().port();
    let mut command = Command::new(env!("CARGO_BIN_EXE_made"));
    for (name, _) in std::env::vars().filter(|(name, _)| name.starts_with("MADE_")) {
        command.env_remove(name);
    }
    command
        .env("MADE_NATS_ENABLED", "false")
        .env("MADE_GRPC_PORT", grpc_port.to_string())
        .env("MADE_HTTP_PORT", http_port.to_string())
        .env("MADE_CEREMONY_STORE_PATH", &configuration.store)
        .env("MADE_GRPC_TLS_MODE", "mutual")
        .env("MADE_GRPC_TLS_CERT_PATH", &configuration.server_certificate)
        .env("MADE_GRPC_TLS_KEY_PATH", &configuration.server_key)
        .env("MADE_GRPC_TLS_CLIENT_CA_PATH", &configuration.ca)
        .env("MADE_AUTH_POLICY_ID", "lifecycle-policy")
        .env("MADE_AUTH_MTLS_PRINCIPALS_PATH", &configuration.principals)
        .env("RUST_LOG", "error")
        .stdout(Stdio::null())
        .stderr(Stdio::inherit());
    drop(grpc_reservation);
    drop(http_reservation);
    (
        ServerProcess(command.spawn().unwrap()),
        grpc_port,
        http_port,
    )
}

async fn wait_until_ready(server: &mut ServerProcess, http_port: u16) {
    let deadline = Instant::now() + Duration::from_secs(10);
    let readiness = loop {
        assert!(
            server.0.try_wait().unwrap().is_none(),
            "server exited before readiness"
        );
        if let Ok(response) = http_get(http_port, "/readyz") {
            if response.starts_with("HTTP/1.1 200") {
                break response;
            }
        }
        assert!(Instant::now() < deadline, "server did not become ready");
        tokio::time::sleep(Duration::from_millis(20)).await;
    };
    assert!(readiness.contains("\"status\":\"ready\""));
    assert!(readiness.contains("in-memory persistence"));
}

async fn verify_grpc_requires_a_business_grant(grpc_port: u16, tls: &TlsMaterial) {
    let channel = Endpoint::from_shared(format!("https://localhost:{grpc_port}"))
        .unwrap()
        .tls_config(
            ClientTlsConfig::new()
                .ca_certificate(Certificate::from_pem(&tls.ca_pem))
                .identity(Identity::from_pem(
                    &tls.client_cert_pem,
                    &tls.client_key_pem,
                ))
                .domain_name("localhost"),
        )
        .unwrap()
        .connect()
        .await
        .unwrap();
    let mut grpc = MadeServiceClient::new(channel);
    let mut request = Request::new(GetStatusRequest {
        include_stats: true,
    });
    request.metadata_mut().insert(
        "x-made-request-id",
        "server-lifecycle-status".parse().unwrap(),
    );
    let status = grpc
        .get_status(request)
        .await
        .expect_err("bootstrap owner needs an explicit business grant");
    assert_eq!(status.code(), Code::PermissionDenied);
}

async fn terminate_and_wait(server: &mut ServerProcess) {
    assert!(Command::new("kill")
        .args(["-TERM", &server.0.id().to_string()])
        .status()
        .unwrap()
        .success());
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Some(status) = server.0.try_wait().unwrap() {
            assert!(status.success(), "graceful shutdown failed: {status}");
            return;
        }
        assert!(
            Instant::now() < deadline,
            "server failed to drain on SIGTERM"
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

#[tokio::test]
async fn server_exposes_both_protocols_and_drains_on_sigterm() {
    let scratch = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tmp");
    std::fs::create_dir_all(&scratch).unwrap();
    let directory = tempfile::tempdir_in(scratch).unwrap();
    let tls = mint_tls();
    let configuration = configure_authorization(directory.path(), &tls);
    let (mut server, grpc_port, http_port) = spawn_server(&configuration);
    wait_until_ready(&mut server, http_port).await;
    let health = http_get(http_port, "/healthz").unwrap();
    assert!(health.contains("\"status\":\"alive\""));
    let metrics = http_get(http_port, "/metrics").unwrap();
    assert!(metrics.contains("made_service_ready 1"));
    verify_grpc_requires_a_business_grant(grpc_port, &tls).await;
    terminate_and_wait(&mut server).await;
    assert!(TcpStream::connect(("127.0.0.1", grpc_port)).is_err());
    assert!(TcpStream::connect(("127.0.0.1", http_port)).is_err());
}
