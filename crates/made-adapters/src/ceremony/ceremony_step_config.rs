//! [`CeremonyStepConfig`] — typed accessor over a ceremony step's handler
//! configuration.
//!
//! Ceremony step handlers are configured through the free-form
//! [`Attributes`] bag carried by `StepHandlerConfig`. Reading that bag
//! with scattered string literals is primitive-obsessed and error-prone,
//! so this view owns the configuration key schema in exactly one place
//! and exposes a validated, fail-fast typed value for each field. Both
//! the deliberation step handler and the participant planner read the
//! step configuration through it, which keeps the schema — including the
//! canonical-vs-legacy agent-kind key — consistent across the adapter.

use made_core::error::DomainError;
use made_core::value_objects::{
    AgentKind, Attributes, NumAgents, Rounds, Specialty, StepHandlerKind, TaskDescription,
};
use serde_json::Value;

use super::ProjectedWinnerFields;

mod evidence;
mod output_contract;

/// Agent kind assumed when a step does not declare one.
const DEFAULT_AGENT_KIND: &str = "noop";

/// Configuration keys recognised on a ceremony step's handler config.
mod key {
    pub(super) const PROMPT: &str = "prompt";
    pub(super) const SPECIALTY: &str = "specialty";
    pub(super) const ROUNDS: &str = "rounds";
    pub(super) const NUM_AGENTS: &str = "num_agents";
    /// Canonical agent-kind key.
    pub(super) const AGENT_KIND: &str = "agent_kind";
    /// Legacy agent-kind key, still accepted for backwards compatibility.
    pub(super) const AGENT_KIND_LEGACY: &str = "agent.kind";
    pub(super) const PARTICIPANTS: &str = "participants";
    /// Whether the step deliberates with the prior transcript in view.
    pub(super) const SEE_PRIOR: &str = "see_prior";
    /// Structured output contract enforced on the step's proposals.
    pub(super) const OUTPUT_CONTRACT: &str = "output_contract";
    pub(super) const PROJECT_WINNER_FIELDS: &str = "project_winner_fields";
}

/// Keys recognised inside the `output_contract` block.
mod contract_key {
    pub(super) const CONTRACT_ID: &str = "contract_id";
    pub(super) const FORMAT: &str = "format";
    pub(super) const REQUIRED_FIELDS: &str = "required_fields";
    pub(super) const ALLOWED_VALUES: &str = "allowed_values";
    pub(super) const JSON_SCHEMA: &str = "json_schema";
    pub(super) const EVIDENCE: &str = "evidence";
}

/// Keys recognised inside the `output_contract.evidence` block.
mod evidence_key {
    pub(super) const CLAIMS_FIELD: &str = "claims_field";
    pub(super) const REFS_FIELD: &str = "refs_field";
    pub(super) const ALLOWED_REFS: &str = "allowed_refs";
    pub(super) const ALLOWED_REFS_FROM_CONTEXT: &str = "allowed_refs_from_context";
    pub(super) const SEMANTIC_SUPPORT: &str = "semantic_support";
}

/// Keys recognised inside the `output_contract.evidence.semantic_support`
/// block.
mod semantic_key {
    pub(super) const MIN_CONFIDENCE: &str = "min_confidence";
    pub(super) const BODIES_FROM_CONTEXT: &str = "bodies_from_context";
}

/// Stable `DomainError` field names surfaced when a value is malformed.
mod field {
    pub(super) const PROMPT: &str = "ceremony_step.config.prompt";
    pub(super) const ROUNDS: &str = "ceremony_step.config.rounds";
    pub(super) const NUM_AGENTS: &str = "ceremony_step.config.num_agents";
    pub(super) const PARTICIPANTS: &str = "ceremony_step.config.participants";
    pub(super) const SEE_PRIOR: &str = "ceremony_step.config.see_prior";
    pub(super) const OUTPUT_CONTRACT: &str = "ceremony_step.config.output_contract";
    pub(super) const PROJECT_WINNER_FIELDS: &str = "ceremony_step.config.project_winner_fields";
    pub(super) const CONTRACT_ID: &str = "ceremony_step.config.output_contract.contract_id";
    pub(super) const CONTRACT_FORMAT: &str = "ceremony_step.config.output_contract.format";
    pub(super) const CONTRACT_REQUIRED_FIELDS: &str =
        "ceremony_step.config.output_contract.required_fields";
    pub(super) const CONTRACT_ALLOWED_VALUES: &str =
        "ceremony_step.config.output_contract.allowed_values";
    pub(super) const CONTRACT_JSON_SCHEMA: &str =
        "ceremony_step.config.output_contract.json_schema";
    pub(super) const CONTRACT_EVIDENCE: &str = "ceremony_step.config.output_contract.evidence";
    pub(super) const EVIDENCE_CLAIMS_FIELD: &str =
        "ceremony_step.config.output_contract.evidence.claims_field";
    pub(super) const EVIDENCE_REFS_FIELD: &str =
        "ceremony_step.config.output_contract.evidence.refs_field";
    pub(super) const EVIDENCE_ALLOWED_REFS: &str =
        "ceremony_step.config.output_contract.evidence.allowed_refs";
    pub(super) const EVIDENCE_CONTEXT_KEY: &str =
        "ceremony_step.config.output_contract.evidence.allowed_refs_from_context";
    pub(super) const EVIDENCE_SEMANTIC: &str =
        "ceremony_step.config.output_contract.evidence.semantic_support";
    pub(super) const SEMANTIC_MIN_CONFIDENCE: &str =
        "ceremony_step.config.output_contract.evidence.semantic_support.min_confidence";
    pub(super) const SEMANTIC_BODIES_KEY: &str =
        "ceremony_step.config.output_contract.evidence.semantic_support.bodies_from_context";
}

/// Default output field carrying the claims array.
const DEFAULT_CLAIMS_FIELD: &str = "claims";
/// Default per-claim field carrying the evidence references.
const DEFAULT_REFS_FIELD: &str = "evidence_refs";

/// A validated, typed view over one ceremony step's handler configuration.
#[derive(Debug, Clone, Copy)]
pub(crate) struct CeremonyStepConfig<'a> {
    attributes: &'a Attributes,
    handler_kind: &'a StepHandlerKind,
}

impl<'a> CeremonyStepConfig<'a> {
    /// Wrap a step's handler-config attributes together with the handler
    /// kind that backs the specialty default.
    pub(crate) fn new(attributes: &'a Attributes, handler_kind: &'a StepHandlerKind) -> Self {
        Self {
            attributes,
            handler_kind,
        }
    }

    /// Required free-text prompt the step deliberates over.
    pub(crate) fn prompt(&self) -> Result<TaskDescription, DomainError> {
        TaskDescription::new(required_string(
            self.attributes.get(key::PROMPT),
            field::PROMPT,
        )?)
    }

    /// Specialty the step deliberates under, defaulting to the handler kind.
    pub(crate) fn specialty(&self) -> Result<Specialty, DomainError> {
        Specialty::new(
            optional_string(self.attributes.get(key::SPECIALTY))
                .unwrap_or_else(|| self.handler_kind.as_str()),
        )
    }

    /// Number of deliberation rounds; the domain default when unset.
    pub(crate) fn rounds(&self) -> Result<Rounds, DomainError> {
        match optional_u32(self.attributes.get(key::ROUNDS), field::ROUNDS)? {
            Some(value) => Rounds::new(value),
            None => Ok(Rounds::default()),
        }
    }

    /// Explicit agent count, if the step declares one.
    pub(crate) fn num_agents(&self) -> Result<Option<NumAgents>, DomainError> {
        optional_u32(self.attributes.get(key::NUM_AGENTS), field::NUM_AGENTS)?
            .map(NumAgents::new)
            .transpose()
    }

    /// Agent kind, resolving the canonical `agent_kind` first, then the
    /// legacy `agent.kind`, and defaulting to noop.
    pub(crate) fn agent_kind(&self) -> Result<AgentKind, DomainError> {
        AgentKind::new(
            optional_string(self.attributes.get(key::AGENT_KIND))
                .or_else(|| optional_string(self.attributes.get(key::AGENT_KIND_LEGACY)))
                .unwrap_or(DEFAULT_AGENT_KIND),
        )
    }

    /// Participant labels declared on the step, in order (possibly empty).
    pub(crate) fn participant_labels(&self) -> Result<Vec<String>, DomainError> {
        let Some(value) = self.attributes.get(key::PARTICIPANTS) else {
            return Ok(Vec::new());
        };
        if value.is_null() {
            return Ok(Vec::new());
        }
        let Some(items) = value.as_array() else {
            return Err(DomainError::InvalidCharacters {
                field: field::PARTICIPANTS,
            });
        };
        items
            .iter()
            .map(|item| {
                item.as_str()
                    .map(str::trim)
                    .filter(|label| !label.is_empty())
                    .map(str::to_owned)
                    .ok_or(DomainError::InvalidCharacters {
                        field: field::PARTICIPANTS,
                    })
            })
            .collect()
    }

    /// Whether the step should deliberate with the prior transcript in
    /// view. Defaults to `true`: a ceremony is a conversation, so a step
    /// builds on what came before unless it is explicitly made blind
    /// (for example, independent estimates before a reveal).
    pub(crate) fn see_prior_steps(&self) -> Result<bool, DomainError> {
        Ok(optional_bool(self.attributes.get(key::SEE_PRIOR), field::SEE_PRIOR)?.unwrap_or(true))
    }

    pub(crate) fn projected_winner_fields(&self) -> Result<ProjectedWinnerFields, DomainError> {
        let Some(value) = self.attributes.get(key::PROJECT_WINNER_FIELDS) else {
            return Ok(ProjectedWinnerFields::default());
        };
        if value.is_null() {
            return Ok(ProjectedWinnerFields::default());
        }
        let items = value
            .as_array()
            .ok_or(DomainError::InvalidCharacters {
                field: field::PROJECT_WINNER_FIELDS,
            })?
            .iter()
            .map(|item| {
                item.as_str()
                    .map(ToOwned::to_owned)
                    .ok_or(DomainError::InvalidCharacters {
                        field: field::PROJECT_WINNER_FIELDS,
                    })
            })
            .collect::<Result<Vec<_>, _>>()?;
        ProjectedWinnerFields::new(items)
    }
}

/// Parse a JSON value as a non-empty-string array (absent/null → empty).
fn string_array(value: Option<&Value>, field: &'static str) -> Result<Vec<String>, DomainError> {
    let Some(value) = value else {
        return Ok(Vec::new());
    };
    if value.is_null() {
        return Ok(Vec::new());
    }
    let Some(items) = value.as_array() else {
        return Err(DomainError::InvalidCharacters { field });
    };
    items
        .iter()
        .map(|item| {
            item.as_str()
                .map(str::trim)
                .filter(|raw| !raw.is_empty())
                .map(str::to_owned)
                .ok_or(DomainError::InvalidCharacters { field })
        })
        .collect()
}

fn required_string(value: Option<&Value>, field: &'static str) -> Result<String, DomainError> {
    let Some(value) = value else {
        return Err(DomainError::EmptyField { field });
    };
    let Some(raw) = value.as_str() else {
        return Err(DomainError::InvalidCharacters { field });
    };
    if raw.trim().is_empty() {
        return Err(DomainError::EmptyField { field });
    }
    Ok(raw.to_owned())
}

fn optional_string(value: Option<&Value>) -> Option<&str> {
    value
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|raw| !raw.is_empty())
}

fn optional_bool(value: Option<&Value>, field: &'static str) -> Result<Option<bool>, DomainError> {
    match value {
        None => Ok(None),
        Some(value) if value.is_null() => Ok(None),
        Some(value) => value
            .as_bool()
            .map(Some)
            .ok_or(DomainError::InvalidCharacters { field }),
    }
}

fn optional_u32(value: Option<&Value>, field: &'static str) -> Result<Option<u32>, DomainError> {
    let Some(value) = value else { return Ok(None) };
    if value.is_null() {
        return Ok(None);
    }
    let Some(raw) = value.as_u64() else {
        return Err(DomainError::InvalidCharacters { field });
    };
    u32::try_from(raw)
        .map(Some)
        .map_err(|_| DomainError::OutOfRange {
            field,
            value: raw as f64,
            min: 0.0,
            max: f64::from(u32::MAX),
        })
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use serde_json::json;

    use super::*;

    fn config(map: BTreeMap<String, Value>) -> (Attributes, StepHandlerKind) {
        (
            Attributes::new(map).unwrap(),
            StepHandlerKind::new("facilitation_prompt").unwrap(),
        )
    }

    #[test]
    fn specialty_defaults_to_handler_kind() {
        let (attributes, handler_kind) = config(BTreeMap::new());
        let step = CeremonyStepConfig::new(&attributes, &handler_kind);

        assert_eq!(step.specialty().unwrap().as_str(), "facilitation_prompt");
    }

    #[test]
    fn explicit_specialty_wins() {
        let (attributes, handler_kind) = config(BTreeMap::from([(
            "specialty".to_owned(),
            json!("facilitator"),
        )]));
        let step = CeremonyStepConfig::new(&attributes, &handler_kind);

        assert_eq!(step.specialty().unwrap().as_str(), "facilitator");
    }

    #[test]
    fn rounds_default_is_one_when_unset() {
        let (attributes, handler_kind) = config(BTreeMap::new());
        let step = CeremonyStepConfig::new(&attributes, &handler_kind);

        assert_eq!(step.rounds().unwrap().get(), 1);
    }

    #[test]
    fn agent_kind_prefers_canonical_key() {
        let (attributes, handler_kind) = config(BTreeMap::from([
            ("agent_kind".to_owned(), json!("vllm")),
            ("agent.kind".to_owned(), json!("openai")),
        ]));
        let step = CeremonyStepConfig::new(&attributes, &handler_kind);

        assert_eq!(step.agent_kind().unwrap().as_str(), "vllm");
    }

    #[test]
    fn agent_kind_falls_back_to_legacy_key() {
        let (attributes, handler_kind) =
            config(BTreeMap::from([("agent.kind".to_owned(), json!("openai"))]));
        let step = CeremonyStepConfig::new(&attributes, &handler_kind);

        assert_eq!(step.agent_kind().unwrap().as_str(), "openai");
    }

    #[test]
    fn agent_kind_defaults_to_noop() {
        let (attributes, handler_kind) = config(BTreeMap::new());
        let step = CeremonyStepConfig::new(&attributes, &handler_kind);

        assert_eq!(step.agent_kind().unwrap().as_str(), "noop");
    }

    #[test]
    fn missing_prompt_is_rejected() {
        let (attributes, handler_kind) = config(BTreeMap::new());
        let step = CeremonyStepConfig::new(&attributes, &handler_kind);

        assert!(matches!(
            step.prompt().unwrap_err(),
            DomainError::EmptyField {
                field: "ceremony_step.config.prompt"
            }
        ));
    }

    #[test]
    fn non_array_participants_are_rejected() {
        let (attributes, handler_kind) = config(BTreeMap::from([(
            "participants".to_owned(),
            json!("facilitator"),
        )]));
        let step = CeremonyStepConfig::new(&attributes, &handler_kind);

        assert!(matches!(
            step.participant_labels().unwrap_err(),
            DomainError::InvalidCharacters {
                field: "ceremony_step.config.participants"
            }
        ));
    }

    #[test]
    fn see_prior_defaults_to_true() {
        let (attributes, handler_kind) = config(BTreeMap::new());
        let step = CeremonyStepConfig::new(&attributes, &handler_kind);

        assert!(step.see_prior_steps().unwrap());
    }

    #[test]
    fn see_prior_can_be_disabled() {
        let (attributes, handler_kind) =
            config(BTreeMap::from([("see_prior".to_owned(), json!(false))]));
        let step = CeremonyStepConfig::new(&attributes, &handler_kind);

        assert!(!step.see_prior_steps().unwrap());
    }

    #[test]
    fn non_bool_see_prior_is_rejected() {
        let (attributes, handler_kind) =
            config(BTreeMap::from([("see_prior".to_owned(), json!("yes"))]));
        let step = CeremonyStepConfig::new(&attributes, &handler_kind);

        assert!(matches!(
            step.see_prior_steps().unwrap_err(),
            DomainError::InvalidCharacters {
                field: "ceremony_step.config.see_prior"
            }
        ));
    }

    #[test]
    fn projected_winner_fields_are_explicit_and_typed() {
        let (attributes, handler_kind) = config(BTreeMap::from([(
            "project_winner_fields".to_owned(),
            json!(["approved", "findings"]),
        )]));
        let step = CeremonyStepConfig::new(&attributes, &handler_kind);

        assert_eq!(
            step.projected_winner_fields()
                .unwrap()
                .iter()
                .map(made_core::value_objects::StepOutputField::as_str)
                .collect::<Vec<_>>(),
            vec!["approved", "findings"]
        );
        let (attributes, handler_kind) = config(BTreeMap::from([(
            "project_winner_fields".to_owned(),
            json!("approved"),
        )]));
        assert!(CeremonyStepConfig::new(&attributes, &handler_kind)
            .projected_winner_fields()
            .is_err());
    }

    #[test]
    fn output_contract_defaults_to_none() {
        let (attributes, handler_kind) = config(BTreeMap::new());
        let step = CeremonyStepConfig::new(&attributes, &handler_kind);

        assert!(step.output_contract().unwrap().is_none());
    }

    #[test]
    fn output_contract_parses_full_shape() {
        let (attributes, handler_kind) = config(BTreeMap::from([(
            "output_contract".to_owned(),
            json!({
                "contract_id": "evidence-bound-decision",
                "format": "json_object",
                "required_fields": ["claims", "decision"],
                "allowed_values": {
                    "decision": ["accept", "reject", "request_changes"],
                    "confidence": ["high", "medium", "low"],
                },
                "json_schema": "{\"type\":\"object\"}",
            }),
        )]));
        let step = CeremonyStepConfig::new(&attributes, &handler_kind);

        let contract = step.output_contract().unwrap().unwrap();

        assert_eq!(contract.contract_id(), "evidence-bound-decision");
        assert!(contract.fields()["claims"].required());
        assert!(contract.fields()["decision"].required());
        assert!(contract.fields()["decision"]
            .allowed_string_values()
            .contains("request_changes"));
        // Constrained but not required: named only under allowed_values.
        assert!(!contract.fields()["confidence"].required());
        assert_eq!(contract.json_schema(), "{\"type\":\"object\"}");
    }

    #[test]
    fn output_contract_requires_contract_id() {
        let (attributes, handler_kind) = config(BTreeMap::from([(
            "output_contract".to_owned(),
            json!({ "required_fields": ["decision"] }),
        )]));
        let step = CeremonyStepConfig::new(&attributes, &handler_kind);

        assert!(matches!(
            step.output_contract().unwrap_err(),
            DomainError::EmptyField {
                field: "ceremony_step.config.output_contract.contract_id"
            }
        ));
    }

    #[test]
    fn output_contract_rejects_unknown_keys() {
        let (attributes, handler_kind) = config(BTreeMap::from([(
            "output_contract".to_owned(),
            json!({ "contract_id": "c1", "require_fields": ["decision"] }),
        )]));
        let step = CeremonyStepConfig::new(&attributes, &handler_kind);

        assert!(matches!(
            step.output_contract().unwrap_err(),
            DomainError::InvalidCharacters {
                field: "ceremony_step.config.output_contract"
            }
        ));
    }

    #[test]
    fn output_contract_rejects_unknown_format() {
        let (attributes, handler_kind) = config(BTreeMap::from([(
            "output_contract".to_owned(),
            json!({ "contract_id": "c1", "format": "yaml" }),
        )]));
        let step = CeremonyStepConfig::new(&attributes, &handler_kind);

        assert!(matches!(
            step.output_contract().unwrap_err(),
            DomainError::InvalidCharacters {
                field: "ceremony_step.config.output_contract.format"
            }
        ));
    }

    #[test]
    fn output_contract_rejects_non_object_block() {
        let (attributes, handler_kind) = config(BTreeMap::from([(
            "output_contract".to_owned(),
            json!("evidence-bound-decision"),
        )]));
        let step = CeremonyStepConfig::new(&attributes, &handler_kind);

        assert!(matches!(
            step.output_contract().unwrap_err(),
            DomainError::InvalidCharacters {
                field: "ceremony_step.config.output_contract"
            }
        ));
    }

    #[test]
    fn output_contract_rejects_non_array_allowed_values() {
        let (attributes, handler_kind) = config(BTreeMap::from([(
            "output_contract".to_owned(),
            json!({ "contract_id": "c1", "allowed_values": { "decision": "accept" } }),
        )]));
        let step = CeremonyStepConfig::new(&attributes, &handler_kind);

        assert!(matches!(
            step.output_contract().unwrap_err(),
            DomainError::InvalidCharacters {
                field: "ceremony_step.config.output_contract.allowed_values"
            }
        ));
    }
}
