//! [`VerifyEvidenceBundleUseCase`] — judge a bundle with no store, no
//! engine and nothing but the file and a verifier.

use std::fmt;
use std::sync::Arc;

use made_core::entities::{EvidenceBundle, SignedEvidenceBundle};
use made_core::error::DomainError;
use made_core::ports::EvidenceVerifierPort;
use made_core::value_objects::EvidenceBundleVerdict;

/// Asks the three questions in the order a reader would: do the records
/// chain, do they end where the bundle says, and does the named key
/// sign that ending. Each is answered on its own so the answer says
/// what was wrong, not only that something was.
pub struct VerifyEvidenceBundleUseCase {
    verifier: Arc<dyn EvidenceVerifierPort>,
}

impl fmt::Debug for VerifyEvidenceBundleUseCase {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("VerifyEvidenceBundleUseCase")
            .finish()
    }
}

impl VerifyEvidenceBundleUseCase {
    #[must_use]
    pub fn new(verifier: Arc<dyn EvidenceVerifierPort>) -> Self {
        Self { verifier }
    }

    /// A signature that cannot be judged (a malformed key, an unknown
    /// scheme) is an error; one that was judged and failed is a verdict.
    pub fn execute(
        &self,
        bundle: &SignedEvidenceBundle,
    ) -> Result<EvidenceBundleVerdict, DomainError> {
        let chain = EvidenceBundle::verify_chain(bundle.records());
        let head_matches = EvidenceBundle::head_of(bundle.records())
            .is_some_and(|recomputed| recomputed == *bundle.head())
            && bundle
                .records()
                .first()
                .is_some_and(|first| first.ceremony_id() == bundle.ceremony_id());
        let signature_valid = self
            .verifier
            .verify(bundle.signature(), &bundle.attestation_message())?;
        Ok(EvidenceBundleVerdict::new(
            chain,
            head_matches,
            signature_valid,
        ))
    }
}

#[cfg(test)]
mod tests {
    use made_core::entities::{AuditRecord, CeremonyEvent};
    use made_core::value_objects::{
        AuditActorKind, EvidenceHead, EvidenceSignature, EvidenceSignatureAlgorithm,
    };
    use serde_json::Value;

    use super::*;
    use crate::services::session_facts;
    use crate::usecases::ceremony_test_support::{definition, now, started_instance};

    /// Accepts exactly the signatures whose value is the message.
    struct EchoVerifier;

    impl EvidenceVerifierPort for EchoVerifier {
        fn verify(
            &self,
            signature: &EvidenceSignature,
            message: &[u8],
        ) -> Result<bool, DomainError> {
            Ok(signature.value() == message)
        }
    }

    fn record() -> AuditRecord {
        let instance = started_instance(&definition());
        let event = CeremonyEvent::CeremonyCompleted(
            made_core::entities::ceremony_events::CeremonyCompleted {
                final_state: made_core::value_objects::StateId::new("DONE").unwrap(),
                completed_at: now(),
            },
        );
        let fact = session_facts::fact(
            &instance,
            event,
            session_facts::party("verifier", AuditActorKind::Engine).unwrap(),
            now(),
        )
        .unwrap();
        AuditRecord::first(fact).unwrap()
    }

    fn signed(head: EvidenceHead, records: Vec<AuditRecord>, honest: bool) -> SignedEvidenceBundle {
        let ceremony_id = records.first().map_or_else(
            || record().ceremony_id().clone(),
            |first| first.ceremony_id().clone(),
        );
        let message = EvidenceBundle::attestation_message(&ceremony_id, &head);
        let value = if honest { message } else { vec![9; 4] };
        SignedEvidenceBundle::from_parts(
            ceremony_id,
            head,
            records,
            EvidenceSignature::new(EvidenceSignatureAlgorithm::Ed25519, vec![1; 32], value)
                .unwrap(),
        )
    }

    #[test]
    fn a_bundle_whose_three_answers_hold_is_sound() {
        let use_case = VerifyEvidenceBundleUseCase::new(Arc::new(EchoVerifier));
        let records = vec![record()];
        let head = EvidenceBundle::head_of(&records).unwrap();
        let verdict = use_case.execute(&signed(head, records, true)).unwrap();
        assert!(verdict.is_sound(), "{verdict:?}");
    }

    #[test]
    fn the_verdict_answers_each_question_on_its_own() {
        let use_case = VerifyEvidenceBundleUseCase::new(Arc::new(EchoVerifier));
        let records = vec![record()];
        let head = EvidenceBundle::head_of(&records).unwrap();

        // A forged signature is reported as exactly that, and nothing else.
        let verdict = use_case
            .execute(&signed(head, records.clone(), false))
            .unwrap();
        assert!(verdict.chain().is_intact());
        assert!(verdict.head_matches());
        assert!(!verdict.signature_valid());
        assert!(!verdict.is_sound());

        // A head the records do not reach is a bundle rewritten after signing.
        let other_head = EvidenceHead::new(head.version().next(), head.hash(), 2);
        let verdict = use_case
            .execute(&signed(other_head, records.clone(), true))
            .unwrap();
        assert!(verdict.chain().is_intact());
        assert!(!verdict.head_matches());
        assert!(verdict.signature_valid());

        // A record whose digest no longer holds breaks the chain; the
        // head computed from it then disagrees with the signed one too.
        let mut json = serde_json::to_value(&records[0]).unwrap();
        json["actor"]["actor_id"] = Value::String("somebody-else".to_owned());
        let tampered: AuditRecord = serde_json::from_value(json).unwrap();
        let verdict = use_case
            .execute(&signed(head, vec![tampered], true))
            .unwrap();
        assert!(!verdict.chain().is_intact());
        assert!(!verdict.is_sound());
    }
}
