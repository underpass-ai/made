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
            object.insert(
                "digest".to_owned(),
                serde_json::Value::String(digest.to_hex()),
            );
        }
        serde_yaml::to_string(&document).map_err(|_| DomainError::InvariantViolated {
            reason: "agentic system document cannot be rendered as YAML",
        })
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
