//! [`ExportEvidenceBundleUseCase`] — one ceremony's journal, verified
//! and signed, ready to be read somewhere the store is not.

use std::fmt;
use std::sync::Arc;

use made_core::entities::{EvidenceBundle, SignedEvidenceBundle};
use made_core::error::DomainError;
use made_core::ports::{CeremonyEventStorePort, EvidenceSignerPort};
use made_core::value_objects::CeremonyId;

use super::ReadWholeCeremonyEventsUseCase;

/// Reads the whole stream, refuses one that does not verify, and signs
/// the head it ends at. The signature is over the head alone because
/// the head's digest already commits to every record before it; what a
/// verifier needs to redo is the chain, which the records carry.
pub struct ExportEvidenceBundleUseCase {
    events: Arc<dyn CeremonyEventStorePort>,
    signer: Arc<dyn EvidenceSignerPort>,
}

impl fmt::Debug for ExportEvidenceBundleUseCase {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ExportEvidenceBundleUseCase")
            .finish()
    }
}

impl ExportEvidenceBundleUseCase {
    #[must_use]
    pub fn new(
        events: Arc<dyn CeremonyEventStorePort>,
        signer: Arc<dyn EvidenceSignerPort>,
    ) -> Self {
        Self { events, signer }
    }

    #[tracing::instrument(
        name = "export_evidence_bundle",
        skip_all,
        fields(ceremony_id = %ceremony_id)
    )]
    pub async fn execute(
        &self,
        ceremony_id: &CeremonyId,
    ) -> Result<SignedEvidenceBundle, DomainError> {
        let records = ReadWholeCeremonyEventsUseCase::new(self.events.clone())
            .execute(ceremony_id)
            .await?;
        let bundle = EvidenceBundle::from_records(records)?;
        if bundle.ceremony_id() != ceremony_id {
            return Err(DomainError::InvariantViolated {
                reason: "the stream holds records of another ceremony",
            });
        }
        let message = EvidenceBundle::attestation_message(bundle.ceremony_id(), &bundle.head());
        let signature = self.signer.sign(&message)?;
        Ok(SignedEvidenceBundle::new(bundle, signature))
    }
}

#[cfg(test)]
mod tests {
    use made_core::entities::{AuditFact, CeremonyEvent};
    use made_core::ports::CeremonyEventStorePort as _;
    use made_core::value_objects::{
        AuditActorKind, EvidenceSignature, EvidenceSignatureAlgorithm, StreamVersion,
    };

    use super::*;
    use crate::services::session_facts;
    use crate::usecases::ceremony_test_support::{
        ceremony_id, definition, now, started_instance, EventStoreFake,
    };

    /// Signs by appending the message to a fixed key: enough to show the
    /// use case signs the right bytes, which is all it owns.
    struct EchoSigner;

    impl EvidenceSignerPort for EchoSigner {
        fn sign(&self, message: &[u8]) -> Result<EvidenceSignature, DomainError> {
            EvidenceSignature::new(
                EvidenceSignatureAlgorithm::Ed25519,
                vec![7; 32],
                message.to_vec(),
            )
        }
    }

    fn fact(ordinal: u64) -> AuditFact {
        let instance = started_instance(&definition());
        let event = CeremonyEvent::CeremonyCompleted(
            made_core::entities::ceremony_events::CeremonyCompleted {
                final_state: made_core::value_objects::StateId::new("DONE").unwrap(),
                completed_at: now(),
            },
        );
        let mut fact = session_facts::fact(
            &instance,
            event,
            session_facts::party("exporter", AuditActorKind::Engine).unwrap(),
            now(),
        )
        .unwrap();
        fact.event_id = made_core::value_objects::EventId::new(format!("fact-{ordinal}")).unwrap();
        fact
    }

    #[tokio::test]
    async fn a_verified_journal_is_bundled_and_its_head_signed() {
        let store = Arc::new(EventStoreFake::default());
        store
            .append(&ceremony_id(), StreamVersion::EMPTY, vec![fact(1), fact(2)])
            .await
            .unwrap();
        let use_case = ExportEvidenceBundleUseCase::new(store, Arc::new(EchoSigner));

        let bundle = use_case.execute(&ceremony_id()).await.unwrap();

        assert_eq!(bundle.ceremony_id(), &ceremony_id());
        assert_eq!(bundle.records().len(), 2);
        assert_eq!(bundle.head().version().value(), 2);
        assert_eq!(bundle.head().record_count(), 2);
        assert_eq!(
            bundle.signature().value(),
            bundle.attestation_message().as_slice(),
            "the signer was handed the attestation message of the head"
        );
    }

    #[tokio::test]
    async fn a_ceremony_with_no_stream_is_not_found() {
        let use_case = ExportEvidenceBundleUseCase::new(
            Arc::new(EventStoreFake::default()),
            Arc::new(EchoSigner),
        );
        assert!(matches!(
            use_case.execute(&ceremony_id()).await,
            Err(DomainError::NotFound { .. })
        ));
    }
}
