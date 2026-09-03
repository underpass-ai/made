/// TLS posture the fixture's gRPC server should present. Materials are
/// PEM-encoded bytes so no temp files or environment mutation are involved.
#[derive(Debug, Clone)]
pub enum TlsServerSetup {
    Server {
        cert: Vec<u8>,
        key: Vec<u8>,
    },
    Mutual {
        cert: Vec<u8>,
        key: Vec<u8>,
        client_ca: Vec<u8>,
    },
}
