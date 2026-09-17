#![cfg(unix)]

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use made_proto::made_service_client::MadeServiceClient;
use made_proto::GetStatusRequest;

struct ServerProcess(Child);

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

#[tokio::test]
async fn server_exposes_both_protocols_and_drains_on_sigterm() {
    let grpc_reservation = TcpListener::bind("127.0.0.1:0").unwrap();
    let http_reservation = TcpListener::bind("127.0.0.1:0").unwrap();
    let grpc_port = grpc_reservation.local_addr().unwrap().port();
    let http_port = http_reservation.local_addr().unwrap().port();
    let mut command = Command::new(env!("CARGO_BIN_EXE_made"));
    for (name, _) in std::env::vars().filter(|(name, _)| name.starts_with("MADE_")) {
        command.env_remove(name);
    }
    // Preserve LLVM_PROFILE_FILE: the child is part of the coverage run.
    command
        .env("MADE_NATS_ENABLED", "false")
        .env("MADE_GRPC_PORT", grpc_port.to_string())
        .env("MADE_HTTP_PORT", http_port.to_string())
        .env("RUST_LOG", "error")
        .stdout(Stdio::null())
        .stderr(Stdio::inherit());
    drop(grpc_reservation);
    drop(http_reservation);
    let mut server = ServerProcess(command.spawn().unwrap());
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
    let health = http_get(http_port, "/healthz").unwrap();
    assert!(health.contains("\"status\":\"alive\""));
    let metrics = http_get(http_port, "/metrics").unwrap();
    assert!(metrics.contains("made_service_ready 1"));
    let mut grpc = MadeServiceClient::connect(format!("http://127.0.0.1:{grpc_port}"))
        .await
        .unwrap();
    let status = grpc
        .get_status(GetStatusRequest {
            include_stats: true,
        })
        .await
        .unwrap()
        .into_inner();
    assert_eq!(status.version, env!("CARGO_PKG_VERSION"));
    assert_eq!(status.health, "healthy");
    assert_eq!(status.stats.unwrap().total_deliberations, 0);
    drop(grpc);
    assert!(Command::new("kill")
        .args(["-TERM", &server.0.id().to_string()])
        .status()
        .unwrap()
        .success());
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Some(status) = server.0.try_wait().unwrap() {
            assert!(status.success(), "graceful shutdown failed: {status}");
            break;
        }
        assert!(
            Instant::now() < deadline,
            "server failed to drain on SIGTERM"
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert!(TcpStream::connect(("127.0.0.1", grpc_port)).is_err());
    assert!(TcpStream::connect(("127.0.0.1", http_port)).is_err());
}
