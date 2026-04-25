//! Shared prompt builders for provider adapters.
//!
//! The Choreographer has a single voice across providers: every
//! adapter speaks the same vocabulary of agent id, specialty, task,
//! proposal, critique, revision. Keeping the prompts in one place
//! ensures that divergence between providers is an explicit
//! decision, not drift.
//!
//! Domain neutrality is enforced by a regression test below that
//! scans the generated text for software-engineering vocabulary
//! (story, backlog, sprint, pull request) — the kind of drift a
//! copy-paste between providers would invite.
//!
//! All functions here are pure: given the same inputs they return
//! the same `String`. No IO, no randomness.

use std::fmt::Write;

use choreo_core::entities::{ExternalContextBundle, TaskConstraints};
use choreo_core::ports::{Critique, DraftRequest};
use choreo_core::value_objects::OutputContract;

pub(super) fn system_prompt_generate(id: &str, specialty: &str) -> String {
    format!(
        "You are a specialist agent in the Underpass Choreographer.\n\
         Your agent id is \"{id}\". Your specialty is \"{specialty}\".\n\
         \n\
         Role:\n\
         - Propose a solution to the task, within your specialty.\n\
         - Keep the proposal concrete, concise, and self-contained.\n\
         - Do not claim capabilities you lack, do not invent facts.\n\
         \n\
         Output contract:\n\
         - Answer only with the proposal body. No preamble, no signature."
    )
}

pub(super) fn system_prompt_critique(id: &str, specialty: &str) -> String {
    format!(
        "You are a specialist agent in the Underpass Choreographer.\n\
         Your agent id is \"{id}\". Your specialty is \"{specialty}\".\n\
         \n\
         Role:\n\
         - Critique a peer's proposal for this task.\n\
         - Flag concrete weaknesses; do not restate the proposal.\n\
         - Prioritise critique that the peer can act on in a revision.\n\
         \n\
         Output contract:\n\
         - Answer only with the critique body. No preamble."
    )
}

pub(super) fn system_prompt_revise(id: &str, specialty: &str) -> String {
    format!(
        "You are a specialist agent in the Underpass Choreographer.\n\
         Your agent id is \"{id}\". Your specialty is \"{specialty}\".\n\
         \n\
         Role:\n\
         - Revise your own proposal in response to the supplied critique.\n\
         - Address the concrete points raised; keep what already works.\n\
         \n\
         Output contract:\n\
         - Answer only with the revised proposal body. No preamble."
    )
}

pub(super) fn user_prompt_generate(request: &DraftRequest) -> String {
    let rubric = serialize_rubric(&request.constraints);
    let diverse_note = if request.diverse {
        "You are one of several peers; propose a distinctive angle rather than a safest-seeming default."
    } else {
        "Propose the option you judge best on the merits."
    };
    let mut prompt = format!(
        "Task:\n{task}\n\n\
         Rubric (opaque constraints to apply):\n{rubric}",
        task = request.task.as_str(),
    );

    if let Some(external_context) = serialize_external_context(request.external_context.as_ref()) {
        let _ = write!(
            prompt,
            "\n\nExternal context bundle (typed, caller-supplied evidence):\n{external_context}"
        );
    }

    if let Some(output_contract) = serialize_output_contract(request.constraints.output_contract())
    {
        let _ = write!(
            prompt,
            "\n\nStructured output contract:\n{output_contract}\n\n\
             Answer only with a JSON object that satisfies this contract. Do not wrap the object in markdown code fences."
        );
    }

    let _ = write!(prompt, "\n\n{diverse_note}\n\nProduce your proposal now.");
    prompt
}

pub(super) fn user_prompt_critique(peer_content: &str, constraints: &TaskConstraints) -> String {
    let rubric = serialize_rubric(constraints);
    let mut prompt = format!(
        "Peer proposal to critique:\n---\n{peer_content}\n---\n\n\
         Rubric (opaque constraints to apply):\n{rubric}"
    );

    if let Some(output_contract) = serialize_output_contract(constraints.output_contract()) {
        let _ = write!(
            prompt,
            "\n\nStructured output contract to enforce while critiquing:\n{output_contract}"
        );
    }

    prompt.push_str("\n\nCritique it now.");
    prompt
}

pub(super) fn user_prompt_revise(own_content: &str, critique: &Critique) -> String {
    format!(
        "Your previous proposal:\n---\n{own_content}\n---\n\n\
         Critique to address:\n---\n{feedback}\n---\n\n\
         Produce the revised proposal now.",
        feedback = critique.feedback,
    )
}

pub(super) fn serialize_rubric(constraints: &TaskConstraints) -> String {
    let rubric = constraints.rubric();
    if rubric.is_empty() {
        "(empty)".to_owned()
    } else {
        serde_json::to_string_pretty(rubric.as_map())
            .unwrap_or_else(|_| "(unrepresentable)".to_owned())
    }
}

fn serialize_external_context(bundle: Option<&ExternalContextBundle>) -> Option<String> {
    bundle.map(|bundle| {
        serde_json::to_string_pretty(bundle).unwrap_or_else(|_| "(unrepresentable)".to_owned())
    })
}

fn serialize_output_contract(contract: Option<&OutputContract>) -> Option<String> {
    contract.map(|contract| {
        serde_json::to_string_pretty(contract).unwrap_or_else(|_| "(unrepresentable)".to_owned())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use choreo_core::entities::{
        ContextItem, ContextReference, ContextSummary, ExternalContextBundle,
    };
    use choreo_core::value_objects::Attributes;
    use serde_json::json;
    use std::collections::BTreeMap;

    #[test]
    fn system_prompt_generate_is_domain_agnostic() {
        let s = system_prompt_generate("a1", "triage");
        assert!(s.contains("a1"));
        assert!(s.contains("triage"));
        for forbidden in ["story", "backlog", "sprint", "pull request"] {
            assert!(
                !s.to_lowercase().contains(forbidden),
                "domain vocabulary leak: {forbidden}"
            );
        }
    }

    #[test]
    fn system_prompts_address_their_specific_role() {
        let g = system_prompt_generate("a", "x");
        let c = system_prompt_critique("a", "x");
        let r = system_prompt_revise("a", "x");
        assert!(g.to_lowercase().contains("propose"));
        assert!(c.to_lowercase().contains("critique"));
        assert!(r.to_lowercase().contains("revise"));
    }

    #[test]
    fn rubric_serialization_handles_empty() {
        let c = TaskConstraints::default();
        assert_eq!(serialize_rubric(&c), "(empty)");
    }

    #[test]
    fn rubric_serialization_returns_json_for_populated_rubric() {
        use choreo_core::value_objects::{NumAgents, Rounds, Rubric};
        use serde_json::json;
        use std::collections::BTreeMap;

        let mut m = BTreeMap::new();
        m.insert("rigor".to_owned(), json!("high"));
        let rubric = Rubric::new(m).unwrap();
        let c = TaskConstraints::new(
            rubric,
            Rounds::default(),
            Some(NumAgents::new(3).unwrap()),
            None,
        );
        let s = serialize_rubric(&c);
        assert!(s.contains("rigor"));
        assert!(s.contains("high"));
    }

    #[test]
    fn user_prompt_generate_embeds_task_and_diversity_hint() {
        use choreo_core::value_objects::TaskDescription;
        let req = DraftRequest {
            task: TaskDescription::new("describe the alert").unwrap(),
            constraints: TaskConstraints::default(),
            diverse: true,
            external_context: None,
        };
        let s = user_prompt_generate(&req);
        assert!(s.contains("describe the alert"));
        assert!(s.to_lowercase().contains("distinctive angle"));
    }

    #[test]
    fn user_prompt_generate_embeds_external_context_bundle_when_present() {
        use choreo_core::value_objects::TaskDescription;

        let req = DraftRequest {
            task: TaskDescription::new("describe the alert").unwrap(),
            constraints: TaskConstraints::default(),
            diverse: false,
            external_context: Some(sample_external_context()),
        };
        let s = user_prompt_generate(&req);
        assert!(s.contains("External context bundle"));
        assert!(s.contains("Bounded caller-supplied context"));
        assert!(s.contains("Primary observation"));
        assert!(s.contains("s3://bucket/evidence.json"));
    }

    #[test]
    fn user_prompt_generate_embeds_structured_output_contract_when_present() {
        use choreo_core::value_objects::{OutputContract, OutputFieldRule, TaskDescription};

        let constraints = TaskConstraints::default().with_output_contract(
            OutputContract::json_object(
                "decision-contract",
                BTreeMap::from([(
                    "decision".to_owned(),
                    OutputFieldRule::new(true, ["emit_event", "escalate"]).unwrap(),
                )]),
            )
            .unwrap(),
        );
        let req = DraftRequest {
            task: TaskDescription::new("describe the alert").unwrap(),
            constraints,
            diverse: false,
            external_context: None,
        };
        let s = user_prompt_generate(&req);
        assert!(s.contains("Structured output contract"));
        assert!(s.contains("decision-contract"));
        assert!(s.contains("emit_event"));
        assert!(s.contains("Answer only with a JSON object"));
    }

    #[test]
    fn user_prompt_critique_embeds_peer_content() {
        let s = user_prompt_critique("the peer's proposal body", &TaskConstraints::default());
        assert!(s.contains("the peer's proposal body"));
    }

    #[test]
    fn user_prompt_critique_mentions_structured_output_contract() {
        use choreo_core::value_objects::{OutputContract, OutputFieldRule};

        let constraints = TaskConstraints::default().with_output_contract(
            OutputContract::json_object(
                "decision-contract",
                BTreeMap::from([(
                    "decision".to_owned(),
                    OutputFieldRule::new(true, ["emit_event", "escalate"]).unwrap(),
                )]),
            )
            .unwrap(),
        );
        let s = user_prompt_critique(r#"{"decision":"emit_event"}"#, &constraints);
        assert!(s.contains("Structured output contract"));
        assert!(s.contains("decision-contract"));
    }

    #[test]
    fn user_prompt_revise_embeds_own_content_and_feedback() {
        let s = user_prompt_revise(
            "v1 body",
            &Critique {
                feedback: "tighten the rubric".to_owned(),
            },
        );
        assert!(s.contains("v1 body"));
        assert!(s.contains("tighten the rubric"));
    }

    fn sample_external_context() -> ExternalContextBundle {
        ExternalContextBundle::new(
            "ctx-1",
            "v1",
            Some(
                ContextSummary::new(
                    "Bounded caller-supplied context",
                    Attributes::new(BTreeMap::from([("origin".to_owned(), json!("caller"))]))
                        .unwrap(),
                )
                .unwrap(),
            ),
            vec![ContextItem::new(
                "item-1",
                "finding",
                "Primary observation",
                Some("A typed context item".to_owned()),
                Attributes::new(BTreeMap::from([("weight".to_owned(), json!(0.9))])).unwrap(),
                vec!["ref-1".to_owned()],
            )
            .unwrap()],
            vec![ContextReference::new(
                "ref-1",
                "s3://bucket/evidence.json",
                Some("Evidence".to_owned()),
                Some("application/json".to_owned()),
                Attributes::empty(),
            )
            .unwrap()],
            Attributes::empty(),
        )
        .unwrap()
    }
}
