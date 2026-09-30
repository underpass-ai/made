//! [`PublishedCeremonyDefinitionYaml`] — a published definition written
//! back in the authoring shape it was published from.
//!
//! The store keeps the definition, not the document an author sent, so
//! reading a publication back means writing a document again. That is
//! only honest if the document names the same content: every rendering
//! is parsed again here and its digest compared with the published one,
//! and a rendering that would change the digest is refused rather than
//! handed out as if it were the publication.

use made_core::entities::{
    CeremonyDefinition, CeremonyDefinitionDraft, PublishedCeremonyDefinition,
};
use made_core::error::DomainError;

use super::{CeremonyDefinitionYaml, DesignedCeremonyYaml};

#[derive(Debug, Default, Clone, Copy)]
pub struct PublishedCeremonyDefinitionYaml;

impl PublishedCeremonyDefinitionYaml {
    /// The authoring YAML that parses back to exactly this publication.
    pub fn render(published: &PublishedCeremonyDefinition) -> Result<String, DomainError> {
        let yaml = DesignedCeremonyYaml::render_draft(&draft_of(published.definition()))?;
        // Recomputed under the scheme the publication was sealed with, so
        // a publication verified under an earlier scheme reads back too.
        let reread = CeremonyDefinitionYaml::parse_str(&yaml)?.digest_under(published.scheme())?;
        if reread != published.digest() {
            return Err(DomainError::InvariantViolated {
                reason: "the published definition cannot be written as authoring YAML without changing its digest",
            });
        }
        Ok(yaml)
    }
}

/// The same content, as the draft the renderer reads. Steps keep the
/// order they were declared in, which is part of what was published.
fn draft_of(definition: &CeremonyDefinition) -> CeremonyDefinitionDraft {
    let mut draft = CeremonyDefinitionDraft::new(
        definition.name().clone(),
        definition.version().clone(),
        definition.description().cloned(),
        definition.inputs().values().cloned(),
        definition.outputs().values().cloned(),
        definition.states().values().cloned(),
        definition.transitions().iter().cloned(),
        definition.steps_in_declaration_order().cloned(),
        definition.guards().values().cloned(),
        definition.roles().values().cloned(),
    )
    .with_max_parallel(definition.max_parallel());
    if let Some(limit) = definition.max_transitions() {
        draft = draft.with_max_transitions(limit);
    }
    if let Some(limit) = definition.max_bounces() {
        draft = draft.with_max_bounces(limit);
    }
    if let Some(timeout) = definition.ceremony_timeout() {
        draft = draft.with_ceremony_timeout(timeout);
    }
    if let Some(timeout) = definition.state_timeout() {
        draft = draft.with_state_timeout(timeout);
    }
    draft
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use made_core::value_objects::{CeremonyStep, DurationMs, StepTimeout};

    use super::*;

    fn published(raw: &str) -> PublishedCeremonyDefinition {
        PublishedCeremonyDefinition::seal(CeremonyDefinitionYaml::parse_str(raw).unwrap()).unwrap()
    }

    /// Every ceremony the repository ships as an example or drives end to
    /// end is one someone could publish, so every one of them must read back to its own
    /// digest.
    #[test]
    fn every_shipped_example_reads_back_to_the_digest_it_was_published_with() {
        let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let roots = [
            workspace.join("api/examples"),
            workspace.join("tests/e2e/ceremonies"),
        ];
        let mut checked = 0;
        for entry in roots.iter().flat_map(|root| walk(root)) {
            let raw = std::fs::read_to_string(&entry).unwrap();
            let Ok(definition) = CeremonyDefinitionYaml::parse_str(&raw) else {
                continue;
            };
            let sealed = PublishedCeremonyDefinition::seal(definition).unwrap();
            let yaml = PublishedCeremonyDefinitionYaml::render(&sealed)
                .unwrap_or_else(|error| panic!("{}: {error}", entry.display()));
            assert_eq!(
                published(&yaml).digest(),
                sealed.digest(),
                "{}",
                entry.display()
            );
            checked += 1;
        }
        assert!(checked > 0, "no ceremony example was found under {roots:?}");
    }

    #[test]
    fn a_definition_without_a_description_is_written_without_one() {
        let sealed = published(
            r#"
version: "2.0"
name: "undescribed"
states:
  - id: WORKING
    initial: true
  - id: DONE
    terminal: true
transitions:
  - from: WORKING
    to: DONE
    trigger: finish
    guards: [worked]
steps:
  - id: work
    state: WORKING
    handler: host_step
guards:
  worked:
    type: automated
    check: "step_status:work:COMPLETED"
roles:
  - id: WORKER
    allowed_actions: [work, finish]
"#,
        );

        let yaml = PublishedCeremonyDefinitionYaml::render(&sealed).unwrap();

        assert!(!yaml.contains("description"), "{yaml}");
        assert_eq!(published(&yaml).digest(), sealed.digest());
    }

    /// A definition built in code can hold what the authoring shape has
    /// no way to say — here, a step with no timeout at all beside one
    /// that has one, where every step the shape writes takes at least
    /// `step_default`. Writing it anyway would hand out a document naming
    /// other content. Steps with different timeouts, by contrast, are
    /// sayable since each may carry its own `timeout_seconds`.
    #[test]
    fn content_the_authoring_shape_cannot_say_is_refused_rather_than_misstated() {
        let (mixed, untimed) = mixed_and_untimed_steps();

        let yaml = PublishedCeremonyDefinitionYaml::render(&mixed).unwrap();
        assert!(yaml.contains("step_default: 10"), "{yaml}");
        assert!(yaml.contains("timeout_seconds: 20"), "{yaml}");
        assert_eq!(published(&yaml).digest(), mixed.digest());

        let error = PublishedCeremonyDefinitionYaml::render(&untimed).unwrap_err();
        assert!(error.to_string().contains("digest"), "{error}");
    }

    /// The same two-step definition sealed twice: once with 10 s and 20 s
    /// step timeouts, once with 10 s and no timeout.
    fn mixed_and_untimed_steps() -> (PublishedCeremonyDefinition, PublishedCeremonyDefinition) {
        let parsed = CeremonyDefinitionYaml::parse_str(
            r#"
version: "1.0"
name: "mixed_timeouts"
states:
  - id: WORKING
    initial: true
  - id: DONE
    terminal: true
transitions:
  - from: WORKING
    to: DONE
    trigger: finish
    guards: [worked]
steps:
  - id: first
    state: WORKING
    handler: host_step
  - id: second
    state: WORKING
    handler: host_step
guards:
  worked:
    type: automated
    check: "all_steps_completed"
roles:
  - id: WORKER
    allowed_actions: [first, second, finish]
"#,
        )
        .unwrap();
        let seal = |second: Option<u64>| {
            let steps = parsed
                .steps_in_declaration_order()
                .enumerate()
                .map(|(position, step)| {
                    let seconds = if position == 0 { Some(10) } else { second };
                    CeremonyStep::new(
                        step.id().clone(),
                        step.state_id().clone(),
                        step.handler_kind().clone(),
                        step.handler_config().clone(),
                        step.retry_policy(),
                        seconds.map(|seconds| {
                            StepTimeout::new(DurationMs::from_millis(seconds * 1000)).unwrap()
                        }),
                    )
                })
                .collect::<Vec<_>>();
            PublishedCeremonyDefinition::seal(
                CeremonyDefinition::new(
                    parsed.name().clone(),
                    parsed.version().clone(),
                    None,
                    parsed.inputs().values().cloned(),
                    parsed.outputs().values().cloned(),
                    parsed.states().values().cloned(),
                    parsed.transitions().iter().cloned(),
                    steps,
                    parsed.guards().values().cloned(),
                    parsed.roles().values().cloned(),
                )
                .unwrap(),
            )
            .unwrap()
        };
        (seal(Some(20)), seal(None))
    }

    fn walk(root: &std::path::Path) -> Vec<PathBuf> {
        let mut found = Vec::new();
        let Ok(entries) = std::fs::read_dir(root) else {
            return found;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                found.extend(walk(&path));
            } else if path
                .extension()
                .is_some_and(|extension| extension == "yaml" || extension == "yml")
            {
                found.push(path);
            }
        }
        found
    }
}
