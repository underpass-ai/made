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

use choreo_core::error::DomainError;
use choreo_core::value_objects::{
    AgentKind, Attributes, NumAgents, Rounds, Specialty, StepHandlerKind, TaskDescription,
};
use serde_json::Value;

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
}

/// Stable `DomainError` field names surfaced when a value is malformed.
mod field {
    pub(super) const PROMPT: &str = "ceremony_step.config.prompt";
    pub(super) const ROUNDS: &str = "ceremony_step.config.rounds";
    pub(super) const NUM_AGENTS: &str = "ceremony_step.config.num_agents";
    pub(super) const PARTICIPANTS: &str = "ceremony_step.config.participants";
    pub(super) const SEE_PRIOR: &str = "ceremony_step.config.see_prior";
}

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
}
