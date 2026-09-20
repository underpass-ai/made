//! One YAML encoding of an agentic system design.
//!
//! A boundary representation, not a second source of truth. The
//! aggregate is what the store holds and what the digest is taken
//! over; this is the shape a person reads and edits, and it is
//! rendered from the aggregate so the two cannot drift.

use made_core::entities::AgenticSystem;
use made_core::error::DomainError;

/// Renders a design as the document an author works on.
#[derive(Debug, Default, Clone, Copy)]
pub struct AgenticSystemYaml;

impl AgenticSystemYaml {
    /// The design as YAML, with its revision, lifecycle and digest at
    /// the top.
    ///
    /// The digest is included because a document without it can be
    /// copied, edited and offered back as though it were the same
    /// design; with it, the difference is visible before anything is
    /// published.
    pub fn render(system: &AgenticSystem) -> Result<String, DomainError> {
        let mut document =
            serde_json::to_value(system).map_err(|_| DomainError::InvariantViolated {
                reason: "agentic system cannot be rendered as a document",
            })?;
        let digest = system.digest()?;
        if let Some(object) = document.as_object_mut() {
            // When it was written is the store's bookkeeping, not the
            // author's document: nobody edits a timestamp, and
            // carrying them here would make one design rendered twice
            // a moment apart look like two.
            object.remove("created_at");
            object.remove("updated_at");
            object.insert(
                "digest".to_owned(),
                serde_json::Value::String(digest.to_hex()),
            );
        }
        readable_digests(&mut document, system);
        serde_yaml::to_string(&document).map_err(|_| DomainError::InvariantViolated {
            reason: "agentic system document cannot be rendered as YAML",
        })
    }
}

/// Show each pin's digest as hex.
///
/// It is thirty-two bytes, and its stored form is an array of
/// numbers. That is right for a payload and unreadable in a document
/// a person is meant to check a pin in: nobody compares two lists of
/// integers by eye.
fn readable_digests(document: &mut serde_json::Value, system: &AgenticSystem) {
    let Some(ceremonies) = document
        .get_mut("ceremonies")
        .and_then(serde_json::Value::as_object_mut)
    else {
        return;
    };
    for (id, composition) in system.ceremonies() {
        let Some(pin) = ceremonies
            .get_mut(id.as_str())
            .and_then(|entry| entry.get_mut("pin"))
            .and_then(serde_json::Value::as_object_mut)
        else {
            continue;
        };
        pin.insert(
            "digest".to_owned(),
            serde_json::Value::String(composition.pin().digest().to_hex()),
        );
    }
}

#[cfg(test)]
mod tests {
    use made_core::value_objects::{
        AgenticSystemId, AttentionPolicy, CeremonyActivation, CeremonyComposition,
        CeremonyDefinitionDigest, CeremonyName, CeremonyVersion, DefinitionPin, LogicalParticipant,
        ParticipantBindingPolicy, ParticipantId, ParticipantKind, Responsibility,
        SupervisionPolicy, SystemCeremonyId, SystemPurpose, SystemRole, SystemRoleId,
        SystemRoleKind,
    };
    use time::OffsetDateTime;

    use super::*;

    fn design() -> AgenticSystem {
        let integrator = SystemRoleId::new("integrator").unwrap();
        AgenticSystem::draft(
            AgenticSystemId::new("delivery").unwrap(),
            SystemPurpose::new("deliver what was asked for").unwrap(),
            integrator.clone(),
            [SystemRole::new(
                integrator.clone(),
                Responsibility::new("drives the system").unwrap(),
                SystemRoleKind::Integrator,
            )],
            [LogicalParticipant::new(
                ParticipantId::new("operator").unwrap(),
                integrator,
                ParticipantKind::Person,
                ParticipantBindingPolicy::default(),
            )],
            [],
            [],
            [CeremonyComposition::new(
                SystemCeremonyId::new("delivery").unwrap(),
                DefinitionPin::new(
                    CeremonyName::new("delivery").unwrap(),
                    CeremonyVersion::new("1.0").unwrap(),
                    CeremonyDefinitionDigest::from_bytes([0x0a; 32]),
                ),
                SystemPurpose::new("does the work").unwrap(),
                [],
                CeremonyActivation::Manual,
                [],
                [],
            )],
            SupervisionPolicy::default(),
            AttentionPolicy::default(),
            OffsetDateTime::UNIX_EPOCH,
        )
        .unwrap()
    }

    #[test]
    fn the_document_carries_the_identity_of_what_it_renders() {
        let system = design();
        let rendered = AgenticSystemYaml::render(&system).unwrap();

        assert!(rendered.contains(&system.digest().unwrap().to_hex()));
        assert!(rendered.contains("deliver what was asked for"));
        assert!(rendered.contains("lifecycle: draft"));
        assert!(!rendered.contains("created_at"));
        assert!(!rendered.contains("updated_at"));
    }

    /// The same design written in two processes a moment apart is one
    /// design, and the document has to say so.
    #[test]
    fn the_document_does_not_change_because_the_clock_did() {
        let earlier = design();
        let later = earlier.edited(OffsetDateTime::UNIX_EPOCH + time::Duration::hours(3));

        assert_eq!(
            AgenticSystemYaml::render(&earlier)
                .unwrap()
                .replace("revision: 1", "revision: 2"),
            AgenticSystemYaml::render(&later).unwrap()
        );
    }

    #[test]
    fn two_renderings_of_one_design_are_the_same_document() {
        let system = design();

        assert_eq!(
            AgenticSystemYaml::render(&system).unwrap(),
            AgenticSystemYaml::render(&system).unwrap()
        );
    }
}
