//! `made_design_ceremony` arguments → `DesignCeremonyRequest`.
//!
//! The nested one. Every other ceremony verb takes a flat object; an
//! authoring intent is a document, so this walks participants, stages
//! and the repeat policy and carries each one across unchanged.
//!
//! Nothing is defaulted here. What an omitted field means is the
//! design use case's decision, and the proto has field presence for
//! exactly the fields where absent and zero are different answers, so
//! an omission reaches the engine as an omission.

use made_mcp_proto::v1 as pb;
use serde_json::{Map, Value};

use super::super::json_to_proto as j2p;

pub(super) fn build_design_ceremony_request(
    args: &Value,
) -> Result<pb::DesignCeremonyRequest, String> {
    let obj = j2p::require_object(args, "tools/call.arguments")?;
    Ok(pb::DesignCeremonyRequest {
        name: j2p::require_str(obj, "name")?.to_owned(),
        version: j2p::optional_str(obj, "version")
            .unwrap_or_default()
            .to_owned(),
        objective: j2p::require_str(obj, "objective")?.to_owned(),
        required_inputs: j2p::string_array(obj, "required_inputs"),
        optional_inputs: j2p::string_array(obj, "optional_inputs"),
        outputs: j2p::string_array(obj, "outputs"),
        participants: participants(obj)?,
        stages: stages(obj)?,
        final_approval: final_approval(obj)?,
        step_timeout_seconds: j2p::optional_present_u64(obj, "step_timeout_seconds")?,
        max_attempts: j2p::optional_present_u32(obj, "max_attempts")?,
        backoff_seconds: j2p::optional_present_u64(obj, "backoff_seconds")?,
        pattern: j2p::optional_str(obj, "pattern")
            .unwrap_or_default()
            .to_owned(),
    })
}

fn participants(obj: &Map<String, Value>) -> Result<Vec<pb::CeremonyDesignParticipant>, String> {
    array(obj, "participants")?
        .iter()
        .map(|value| {
            let participant = j2p::require_object(value, "participants[]")?;
            Ok(pb::CeremonyDesignParticipant {
                role_id: j2p::require_str(participant, "role_id")?.to_owned(),
                capabilities: j2p::string_array(participant, "capabilities"),
            })
        })
        .collect()
}

fn stages(obj: &Map<String, Value>) -> Result<Vec<pb::CeremonyDesignStage>, String> {
    array(obj, "stages")?
        .iter()
        .map(|value| {
            let stage = j2p::require_object(value, "stages[]")?;
            Ok(pb::CeremonyDesignStage {
                id: j2p::require_str(stage, "id")?.to_owned(),
                owner_role_id: j2p::require_str(stage, "owner_role_id")?.to_owned(),
                instructions: j2p::require_str(stage, "instructions")?.to_owned(),
                handler: j2p::optional_str(stage, "handler")
                    .unwrap_or_default()
                    .to_owned(),
                see_prior: stage.get("see_prior").and_then(Value::as_bool),
                num_agents: j2p::optional_present_u64(stage, "num_agents")?,
                review_rounds: j2p::optional_u64(stage, "review_rounds")?,
                repeat: repeat(stage)?,
            })
        })
        .collect()
}

fn repeat(stage: &Map<String, Value>) -> Result<Option<pb::CeremonyDesignRepeat>, String> {
    let Some(value) = stage.get("repeat") else {
        return Ok(None);
    };
    let repeat = j2p::require_object(value, "stages[].repeat")?;
    Ok(Some(pb::CeremonyDesignRepeat {
        max_iterations: j2p::optional_u32(repeat, "max_iterations")?,
        output_field: j2p::require_str(repeat, "output_field")?.to_owned(),
        // Absent is null rather than missing: the field is what ends
        // the repetition, and a policy that tests nothing would run to
        // its cap every time.
        equals: Some(j2p::json_to_pb_value(
            repeat.get("equals").unwrap_or(&Value::Null),
        )),
    }))
}

fn final_approval(
    obj: &Map<String, Value>,
) -> Result<Option<pb::CeremonyDesignFinalApproval>, String> {
    let Some(value) = obj.get("final_approval") else {
        return Ok(None);
    };
    let approval = j2p::require_object(value, "final_approval")?;
    Ok(Some(pb::CeremonyDesignFinalApproval {
        role_id: j2p::require_str(approval, "role_id")?.to_owned(),
        guard_name: j2p::optional_str(approval, "guard_name")
            .unwrap_or_default()
            .to_owned(),
        trigger: j2p::optional_str(approval, "trigger")
            .unwrap_or_default()
            .to_owned(),
    }))
}

/// A declared array, or an empty one. Whether it may be empty is the
/// engine's question, not this layer's: refusing here would put the
/// rule in two places and let the two disagree.
fn array<'a>(obj: &'a Map<String, Value>, key: &str) -> Result<&'a [Value], String> {
    match obj.get(key) {
        None | Some(Value::Null) => Ok(&[]),
        Some(Value::Array(values)) => Ok(values),
        Some(_) => Err(format!("field `{key}` must be an array")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn intent() -> Value {
        json!({
            "name": "art_review",
            "objective": "Compose one candidate and ask the artist to accept it.",
            "outputs": ["candidate_review"],
            "participants": [
                { "role_id": "WORKER", "capabilities": ["respond_to_intervention"] }
            ],
            "stages": [
                {
                    "id": "compose",
                    "owner_role_id": "WORKER",
                    "instructions": "Compose the candidate.",
                    "repeat": { "max_iterations": 3, "output_field": "ready", "equals": true }
                }
            ]
        })
    }

    /// An omission crosses the wire as an omission: the engine, and
    /// only the engine, says what it means.
    #[test]
    fn an_omitted_field_is_absent_rather_than_zero() {
        let request = build_design_ceremony_request(&intent()).expect("the intent is accepted");

        assert_eq!(request.version, "");
        assert_eq!(request.step_timeout_seconds, None);
        assert_eq!(request.max_attempts, None);
        assert_eq!(request.backoff_seconds, None);
        assert_eq!(request.stages[0].num_agents, None);
        assert_eq!(request.stages[0].see_prior, None);
        assert_eq!(request.stages[0].handler, "");
        assert!(request.final_approval.is_none());
    }

    /// A zero the caller wrote is a zero the engine sees, which is
    /// how `backoff_seconds: 0` stays different from leaving it out.
    #[test]
    fn a_zero_the_caller_wrote_survives_the_crossing() {
        let mut value = intent();
        value["backoff_seconds"] = json!(0);
        value["stages"][0]["num_agents"] = json!(0);
        value["stages"][0]["see_prior"] = json!(false);

        let request = build_design_ceremony_request(&value).expect("the intent is accepted");

        assert_eq!(request.backoff_seconds, Some(0));
        assert_eq!(request.stages[0].num_agents, Some(0));
        assert_eq!(request.stages[0].see_prior, Some(false));
    }

    #[test]
    fn the_repeat_policy_carries_its_cap_and_the_value_that_ends_it() {
        let request = build_design_ceremony_request(&intent()).expect("the intent is accepted");
        let repeat = request.stages[0]
            .repeat
            .as_ref()
            .expect("the stage repeats");

        assert_eq!(repeat.max_iterations, 3);
        assert_eq!(repeat.output_field, "ready");
        assert_eq!(
            repeat.equals,
            Some(j2p::json_to_pb_value(&json!(true))),
            "the value that ends the repetition is carried, not described"
        );
    }

    #[test]
    fn a_pattern_crosses_on_reserved_field_fourteen_without_stages() {
        let mut value = intent();
        value.as_object_mut().unwrap().remove("stages");
        value["pattern"] = json!("roundtable_fixed_order");

        let request = build_design_ceremony_request(&value).expect("the preset is accepted");

        assert_eq!(request.pattern, "roundtable_fixed_order");
        assert!(request.stages.is_empty());
    }
}
