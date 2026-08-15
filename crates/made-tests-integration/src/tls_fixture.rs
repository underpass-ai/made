//! `rcgen`-backed TLS fixture for the in-process gRPC handshake tests.
//!
//! Mints a single self-signed CA and two leaf certificates (server +
//! client) under it. All material lives in memory as PEM bytes; the
//! caller hands the bytes to `tonic::transport::{ServerTlsConfig,
//! ClientTlsConfig}` directly, so no temp files and no process env
//! mutation are needed.
//!
//! The CA is "self-signed" in the sense that its issuer == subject;
//! both leaves are signed by it. `tonic` validates leaf chains via
//! `Certificate::from_pem` + `client_ca_root` / `ca_certificate`, so
//! presenting the CA PEM to one side and the leaf identity to the
//! other is enough.

use rcgen::{CertificateParams, DistinguishedName, DnType, IsCa, KeyUsagePurpose, SanType};

pub struct MintedTls {
    pub ca_pem: Vec<u8>,
    pub server_cert_pem: Vec<u8>,
    pub server_key_pem: Vec<u8>,
    pub client_cert_pem: Vec<u8>,
    pub client_key_pem: Vec<u8>,
}

/// Mint a fresh CA + server leaf (with `server_san` as a DNS SAN) +
/// client leaf, all in memory, all PEM-encoded.
#[must_use]
pub fn mint_tls(server_san: &str) -> MintedTls {
    let mut ca_params = CertificateParams::new(Vec::<String>::new()).expect("ca params");
    ca_params.is_ca = IsCa::Ca(rcgen::BasicConstraints::Unconstrained);
    ca_params.key_usages.push(KeyUsagePurpose::KeyCertSign);
    ca_params.key_usages.push(KeyUsagePurpose::CrlSign);
    let mut ca_dn = DistinguishedName::new();
    ca_dn.push(DnType::CommonName, "made-test-ca");
    ca_params.distinguished_name = ca_dn;
    let ca_key = rcgen::KeyPair::generate().expect("ca keypair");
    let ca_cert = ca_params.self_signed(&ca_key).expect("ca self-signed");

    let mut server_params =
        CertificateParams::new(vec![server_san.to_owned()]).expect("server params");
    server_params
        .subject_alt_names
        .push(SanType::DnsName(server_san.try_into().expect("server SAN")));
    let mut server_dn = DistinguishedName::new();
    server_dn.push(DnType::CommonName, server_san);
    server_params.distinguished_name = server_dn;
    let server_key = rcgen::KeyPair::generate().expect("server keypair");
    let server_cert = server_params
        .signed_by(&server_key, &ca_cert, &ca_key)
        .expect("server signed by ca");

    let mut client_params = CertificateParams::new(Vec::<String>::new()).expect("client params");
    let mut client_dn = DistinguishedName::new();
    client_dn.push(DnType::CommonName, "made-test-client");
    client_params.distinguished_name = client_dn;
    let client_key = rcgen::KeyPair::generate().expect("client keypair");
    let client_cert = client_params
        .signed_by(&client_key, &ca_cert, &ca_key)
        .expect("client signed by ca");

    MintedTls {
        ca_pem: ca_cert.pem().into_bytes(),
        server_cert_pem: server_cert.pem().into_bytes(),
        server_key_pem: server_key.serialize_pem().into_bytes(),
        client_cert_pem: client_cert.pem().into_bytes(),
        client_key_pem: client_key.serialize_pem().into_bytes(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mint_tls_emits_three_distinct_pem_certs_and_two_keys() {
        let tls = mint_tls("localhost");
        // PEM headers present
        let starts_cert = b"-----BEGIN CERTIFICATE-----";
        let starts_key = b"-----BEGIN PRIVATE KEY-----";
        assert!(tls.ca_pem.starts_with(starts_cert));
        assert!(tls.server_cert_pem.starts_with(starts_cert));
        assert!(tls.client_cert_pem.starts_with(starts_cert));
        assert!(tls.server_key_pem.starts_with(starts_key));
        assert!(tls.client_key_pem.starts_with(starts_key));
        // Distinct identities
        assert_ne!(tls.server_cert_pem, tls.client_cert_pem);
        assert_ne!(tls.server_cert_pem, tls.ca_pem);
    }
}
