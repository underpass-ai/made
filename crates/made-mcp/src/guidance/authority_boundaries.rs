//! What an agent may never infer, whatever it was told.
//!
//! Its own file because every one of these is a rule and a named
//! mistake, and the mistake is the useful half: a boundary stated
//! without the inference it forbids reads as advice, and advice is
//! what an agent talks itself out of.

use serde_json::{json, Value};

pub(super) fn base_agent_authority_boundaries() -> Vec<Value> {
    vec![
        json!({
            "rule": "Human guard approval requires a person's explicit current authorization.",
            "forbidden_inference": "Silence, prior approval, an agent recommendation, or operational convenience."
        }),
        json!({
            "rule": "Interventions coordinate requests but grant no external mutation authority.",
            "forbidden_inference": "An action request is permission to alter another system."
        }),
        json!({
            "rule": "A delivered intervention is proved by its sealed acknowledgement, not by a participant's own status label.",
            "forbidden_inference": "An `intervention_delivered` or `intervention_answered` activity label, or a queued route, means somebody saw it."
        }),
        json!({
            "rule": "A lease from a pull is an offer; acknowledge what you were handed before acting on it, and answer naming the delivery you were handed.",
            "forbidden_inference": "Holding a lease is the same as having told anybody you have it, or `busy` is a polite way to say `received`."
        }),
        json!({
            "rule": "Evidence must come from an actual authorized source and remain attributable.",
            "forbidden_inference": "An empty, inaccessible, or imagined source is evidence."
        }),
        json!({
            "rule": "Integrator is a business responsibility, not a privilege; declared role actions and human guards still apply.",
            "forbidden_inference": "A role named integrator may approve for a person, bypass independent review, or prove host-agent activity."
        }),
        json!({
            "rule": "Execution profiles are host-owned evidence attached to delegated claims, not ceremony authority or provider identity.",
            "forbidden_inference": "A requested model is the actual model, or a Codex/Claude agent id is a MADE role id."
        }),
    ]
}
