//! `made-mcp keygen`, `export-evidence` and `verify-evidence`: a
//! ceremony's journal as a file somebody else can judge.
//!
//! The hash chain inside a store proves the records were not altered in
//! that store. A bundle carries the same records out, verified, with the
//! head signed by a key the operator holds; the verifier needs the file
//! and, if they want to pin the signer, the public key — no store, no
//! engine, no MADE account.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use made_adapters::evidence::{Ed25519EvidenceSigner, Ed25519EvidenceVerifier};
use made_app::usecases::VerifyEvidenceBundleUseCase;
use made_core::entities::SignedEvidenceBundle;
use made_core::value_objects::CeremonyId;
use made_embedded::EmbeddedMade;

mod evidence_export_receipt;
mod evidence_verification;

pub use evidence_export_receipt::EvidenceExportReceipt;
pub use evidence_verification::EvidenceVerification;

/// Draw a new signing key and keep its seed at `path`, owner-only.
/// Returns the public key to hand to whoever will verify exports.
pub fn keygen(path: &Path) -> Result<String, String> {
    let signer = Ed25519EvidenceSigner::generate().map_err(|error| error.to_string())?;
    signer
        .write_seed_file(path)
        .map_err(|error| format!("could not write `{}`: {error}", path.display()))?;
    Ok(signer.public_key_hex())
}

/// The public key of the seed at `path`, for a setup that wants to
/// print it again without touching the seed.
pub fn public_key(path: &Path) -> Result<String, String> {
    Ed25519EvidenceSigner::from_seed_file(path)
        .map(|signer| signer.public_key_hex())
        .map_err(|error| format!("could not read the key at `{}`: {error}", path.display()))
}

/// Read one ceremony's journal from `store`, verify it, sign its head
/// with the key at `key_path` and write the bundle to `out`.
pub async fn export(
    store: &Path,
    ceremony_id: &str,
    key_path: &Path,
    out: &Path,
) -> Result<EvidenceExportReceipt, String> {
    let ceremony_id = CeremonyId::new(ceremony_id).map_err(|error| error.to_string())?;
    let signer = Ed25519EvidenceSigner::from_seed_file(key_path)
        .map_err(|error| format!("could not use the key at `{}`: {error}", key_path.display()))?;
    let public_key_hex = signer.public_key_hex();
    let made = EmbeddedMade::open(store)
        .map_err(|error| format!("could not open the store at `{}`: {error}", store.display()))?;
    let bundle = made
        .export_evidence_bundle(&ceremony_id, Arc::new(signer))
        .await
        .map_err(|error| {
            format!(
                "could not export ceremony `{}`: {error}",
                ceremony_id.as_str()
            )
        })?;
    let json = bundle.to_json().map_err(|error| error.to_string())?;
    write_new(out, &json)?;
    Ok(EvidenceExportReceipt::new(
        ceremony_id,
        bundle.head().version().value(),
        bundle.head().record_count(),
        public_key_hex,
        PathBuf::from(out),
    ))
}

/// Judge the bundle at `file`. With `expected_public_key`, also say
/// whether it was signed by that key.
pub fn verify(
    file: &Path,
    expected_public_key: Option<&str>,
) -> Result<EvidenceVerification, String> {
    let json = std::fs::read_to_string(file)
        .map_err(|error| format!("could not read `{}`: {error}", file.display()))?;
    let bundle = SignedEvidenceBundle::from_json(&json).map_err(|error| error.to_string())?;
    let verdict = VerifyEvidenceBundleUseCase::new(Arc::new(Ed25519EvidenceVerifier))
        .execute(&bundle)
        .map_err(|error| format!("the bundle cannot be judged: {error}"))?;
    let public_key_hex = bundle.signature().public_key_hex();
    let key_matches =
        expected_public_key.map(|expected| expected.trim().eq_ignore_ascii_case(&public_key_hex));
    Ok(EvidenceVerification::new(
        bundle.ceremony_id().clone(),
        bundle.head().version().value(),
        bundle.head().record_count(),
        public_key_hex,
        verdict,
        key_matches,
    ))
}

fn write_new(path: &Path, contents: &str) -> Result<(), String> {
    if path.exists() {
        return Err(format!(
            "`{}` already exists; evidence files are not overwritten",
            path.display()
        ));
    }
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)
                .map_err(|error| format!("could not create `{}`: {error}", parent.display()))?;
        }
    }
    std::fs::write(path, contents)
        .map_err(|error| format!("could not write `{}`: {error}", path.display()))
}

#[cfg(test)]
mod tests {
    use made_adapters::clock::SystemClock;
    use made_adapters::sqlite::SqliteAuthorizationPolicyStore;
    use made_app::authorization::AuthorizationPolicyAdministrationService;
    use made_core::value_objects::{
        AuthenticatedPrincipal, AuthenticationMethod, AuthorizationAction, AuthorizationGrant,
        AuthorizationGrantId, AuthorizationGrantIssuer, AuthorizationPolicyId, AuthorizationScope,
        DelegationDepth, PrincipalId, PrincipalKind,
    };
    use serde_json::{json, Value};

    use super::*;
    use crate::MadeMcpServer;

    const POLICY: &str = "evidence-policy";
    const HOST: &str = "evidence-host";
    const CEREMONY_YAML: &str = r#"
version: "1.0"
name: "evidence_export"
states:
  - id: OPEN
    initial: true
  - id: DONE
    terminal: true
transitions:
  - from: OPEN
    to: DONE
    trigger: finish
steps:
  - id: work
    state: OPEN
    handler: embedded_noop
roles:
  - id: WORKER
    allowed_actions:
      - work
      - finish
"#;

    async fn started_session(store: &Path) -> MadeMcpServer {
        let administration = AuthorizationPolicyAdministrationService::new(
            AuthorizationPolicyId::new(POLICY).unwrap(),
            Arc::new(SqliteAuthorizationPolicyStore::open(store).unwrap()),
            Arc::new(SystemClock::new()),
        );
        let owner = AuthenticatedPrincipal::new(
            PrincipalId::new(HOST).unwrap(),
            PrincipalKind::TrustedHost,
            AuthenticationMethod::LocalHostPolicy,
        )
        .unwrap();
        administration
            .open(owner.clone(), Vec::new())
            .await
            .unwrap();
        administration
            .issue(
                &owner,
                AuthorizationGrant::new(
                    AuthorizationGrantId::new("evidence-grant").unwrap(),
                    owner.id().clone(),
                    [
                        AuthorizationAction::PublishCeremonyDefinition,
                        AuthorizationAction::StartPublishedCeremony,
                        AuthorizationAction::ApplyCeremonyTransition,
                    ],
                    AuthorizationScope::Global,
                    (time::OffsetDateTime::UNIX_EPOCH, None),
                    DelegationDepth::none(),
                    AuthorizationGrantIssuer::direct(owner.clone()),
                )
                .unwrap(),
            )
            .await
            .unwrap();
        let server = MadeMcpServer::embedded_sqlite_authorized(store, POLICY, HOST).unwrap();
        server.initialize_backend().await.unwrap();
        for (id, tool, arguments) in [
            (
                1,
                "made_publish_ceremony_definition",
                json!({"definition_yaml": CEREMONY_YAML}),
            ),
            (
                2,
                "made_start_published_ceremony",
                json!({
                    "ceremony": "evidence_export", "version": "1.0", "ceremony_id": "export-1",
                    "actor_id": "operator", "actor_kind": "service", "context": {},
                }),
            ),
            (
                3,
                "made_apply_ceremony_transition",
                json!({"ceremony_id": "export-1", "trigger": "finish", "actor_kind": "service"}),
            ),
        ] {
            let request = json!({
                "jsonrpc": "2.0", "id": id, "method": "tools/call",
                "params": {"name": tool, "arguments": arguments},
            });
            let response = server.handle_json_line(&request.to_string()).await.unwrap();
            let response: Value = serde_json::from_str(&response).unwrap();
            assert_ne!(response["result"]["isError"], true, "{response}");
        }
        server
    }

    #[tokio::test]
    async fn an_export_verifies_anywhere_and_a_tampered_one_does_not() {
        let directory = tempfile::tempdir().unwrap();
        let store = directory.path().join("ceremonies.sqlite3");
        let _server = started_session(&store).await;
        let key = directory.path().join("keys/evidence.key");
        let out = directory.path().join("export-1.evidence.json");

        let public_key = keygen(&key).unwrap();
        assert_eq!(public_key.len(), 64);
        assert_eq!(super::public_key(&key).unwrap(), public_key);
        assert!(keygen(&key).is_err(), "a key is never silently replaced");

        let receipt = export(&store, "export-1", &key, &out).await.unwrap();
        assert_eq!(receipt.ceremony_id().as_str(), "export-1");
        assert_eq!(receipt.public_key_hex(), public_key);
        assert_eq!(receipt.record_count(), 3);
        assert_eq!(receipt.head_version(), 3);
        assert!(receipt
            .lines()
            .iter()
            .any(|line| line.contains(&public_key)));
        assert!(export(&store, "export-1", &key, &out).await.is_err());

        // Judged with nothing but the file.
        let verification = verify(&out, Some(&public_key)).unwrap();
        assert!(verification.is_sound(), "{:?}", verification.lines());
        assert_eq!(verification.key_matches(), Some(true));
        let unpinned = verify(&out, None).unwrap();
        assert!(unpinned.is_sound());
        assert_eq!(unpinned.key_matches(), None);

        // The wrong expected key is reported, and the rest still holds.
        let other = verify(&out, Some(&"0".repeat(64))).unwrap();
        assert!(!other.is_sound());
        assert!(other.verdict().is_sound());
        assert_eq!(other.key_matches(), Some(false));

        // A record edited after signing breaks the chain and the head.
        let mut file: Value =
            serde_json::from_str(&std::fs::read_to_string(&out).unwrap()).unwrap();
        // Authorized records travel enveloped: the fact is under `record`.
        let record = &mut file["records"][1];
        let inner = if record.get("record").is_some() {
            &mut record["record"]
        } else {
            record
        };
        inner["actor"]["actor_id"] = Value::String("forger".to_owned());
        let tampered = directory.path().join("tampered.json");
        std::fs::write(&tampered, file.to_string()).unwrap();
        let verification = verify(&tampered, Some(&public_key)).unwrap();
        assert!(!verification.is_sound());
        assert!(!verification.verdict().chain().is_intact());

        // A forged signature is reported as exactly that.
        let mut file: Value =
            serde_json::from_str(&std::fs::read_to_string(&out).unwrap()).unwrap();
        file["signature"]["value"] = Value::String("ab".repeat(64));
        let forged = directory.path().join("forged.json");
        std::fs::write(&forged, file.to_string()).unwrap();
        let verification = verify(&forged, None).unwrap();
        assert!(verification.verdict().chain().is_intact());
        assert!(verification.verdict().head_matches());
        assert!(!verification.verdict().signature_valid());
        assert!(verification
            .lines()
            .iter()
            .any(|line| line.contains("verdict: NOT sound")));

        // Not a bundle at all.
        let noise = directory.path().join("noise.json");
        std::fs::write(&noise, "{}").unwrap();
        assert!(verify(&noise, None).is_err());
    }
}
