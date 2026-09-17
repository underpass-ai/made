//! A second complete session exercises values that the default script omits.
//! It has fresh stores so its richer history does not rewrite the original golden.

use super::*;

fn opening_context() -> Value {
    json!({"ticket": "INC-optional", "severity": 2, "approved": false,
           "attachment": {"ref": "artifact:review", "score": 1.25}})
}

fn provenance() -> Value {
    json!({"source_intervention_id": "what-happened",
           "source_response_role_id": "OBSERVER", "selected_role_id": "OBSERVER"})
}

fn rich_script() -> Vec<(&'static str, Value)> {
    let mut script = session_script();
    for (tool, arguments) in &mut script {
        match *tool {
            "made_design_ceremony" => {
                arguments["version"] = json!("2.0");
                arguments["step_timeout_seconds"] = json!(321);
                arguments["max_attempts"] = json!(4);
                arguments["backoff_seconds"] = json!(3);
                for stage in arguments["stages"].as_array_mut().unwrap() {
                    stage["handler"] = json!("host_callback");
                }
                arguments["stages"][0]["see_prior"] = json!(false);
                arguments["stages"][0]["num_agents"] = json!(1);
                arguments["stages"][0]["review_rounds"] = json!(0);
                arguments["stages"][0]["exit_guards"] = json!([
                    {"kind": "output_field", "step": "draft_options",
                     "output_field": "decision=key", "equals": {"label": "left=right"}},
                    {"kind": "step_repeat_exhausted", "step": "draft_options"}
                ]);
                arguments["final_approval"]["guard_name"] = json!("release_approved");
                arguments["final_approval"]["trigger"] = json!("publish_outcome");
            }
            "made_start_published_ceremony" | "made_run_ceremony" => {
                arguments["context"] = opening_context();
                arguments["actor_kind"] = json!("engine");
                if *tool == "made_run_ceremony" {
                    arguments["lease_ttl_ms"] = json!(17_000);
                }
            }
            "made_run_ceremony_step" => arguments["lease_ttl_ms"] = json!(19_000),
            "made_request_ceremony_intervention"
                if arguments["intervention_id"] == "inspect-metrics" =>
            {
                arguments["kind"] = json!("action");
                arguments["provenance"] = provenance();
                arguments["details"] = json!({"selected": true, "ticket": "INC-optional"});
            }
            _ => {}
        }
    }
    script
}

async fn checked(arms: &ParityArms, id: u64, tool: &str, arguments: Value) -> Value {
    let (wire, embedded) = arms.call(id, tool, &arguments).await;
    assert!(!failed(&wire), "{tool} gRPC refused {arguments}: {wire:#}");
    assert!(
        !failed(&embedded),
        "{tool} embedded refused {arguments}: {embedded:#}"
    );
    assert_same_answer(tool, &wire, &embedded);
    embedded
}

#[tokio::test]
async fn filled_optionals_and_rotated_enums_match_on_both_backends() {
    drive_rich_session(&ParityArms::start().await).await;
}

#[tokio::test]
async fn filled_optionals_and_rotated_enums_match_on_shipped_sqlite() {
    drive_rich_session(&ParityArms::start_on_the_shipped_store().await).await;
}

async fn drive_rich_session(arms: &ParityArms) {
    for (index, (tool, arguments)) in rich_script().into_iter().enumerate() {
        let expects_provenance = tool == "made_request_ceremony_intervention"
            && arguments["intervention_id"] == "inspect-metrics";
        let answer = checked(arms, 1_000 + index as u64, tool, arguments).await;
        if tool == "made_design_ceremony" {
            assert_design_optionals(structured(&answer));
        }
        if expects_provenance {
            let items = structured(&answer)["interventions"].as_array().unwrap();
            let item = items
                .iter()
                .find(|item| item["intervention_id"] == "inspect-metrics")
                .expect("the intervention carrying provenance is returned");
            assert_eq!(item["provenance"], provenance());
            assert_eq!(item["kind"], "action");
            assert_eq!(item["request"]["details"]["selected"], true);
        }
    }
    assert_opening_contexts(arms).await;
    assert_reason_variants(arms).await;
    assert_completion_variants(arms).await;

    let status = checked(
        arms,
        1_900,
        "made_get_status",
        json!({"include_stats": false}),
    )
    .await;
    assert_eq!(structured(&status)["stats"], Value::Null);
    checked(
        arms,
        1_901,
        "made_diff_ceremony_definitions",
        json!({
            "before": {"definition_yaml": ALTERED_CEREMONY},
            "after": {"ceremony": "parity_published", "version": "1.0"}
        }),
    )
    .await;
    let transcript = checked(
        arms,
        1_902,
        "made_get_ceremony_transcript",
        json!({"ceremony_id": SESSION_ID}),
    )
    .await;
    assert_eq!(structured(&transcript)["entry_count"], 2);
    assert_eq!(structured(&transcript)["entries"][1]["step_id"], "handoff");
    assert_pattern_optional(arms).await;
}

async fn assert_pattern_optional(arms: &ParityArms) {
    let arguments = json!({
        "name": "parity_roundtable", "objective": "Review one bounded question.",
        "outputs": ["discussion"], "stages": [],
        "participants": [{"role_id": "FACILITATOR"}, {"role_id": "REVIEWER"}],
        "pattern": "roundtable_fixed_order"
    });
    let answer = checked(arms, 1_950, "made_design_ceremony", arguments).await;
    assert_eq!(structured(&answer)["publishable"], true);
    let definition = made_adapters::yaml::CeremonyDefinitionYaml::parse_str(
        structured(&answer)["definition_yaml"].as_str().unwrap(),
    )
    .unwrap();
    let owners = definition
        .steps_in_declaration_order()
        .map(|step| {
            definition
                .role_id_for_step(step.id())
                .unwrap()
                .as_str()
                .to_owned()
        })
        .collect::<Vec<_>>();
    assert_eq!(owners, ["FACILITATOR", "REVIEWER"]);
}

fn assert_design_optionals(answer: &Value) {
    let yaml = answer["definition_yaml"].as_str().unwrap();
    let definition = made_adapters::yaml::CeremonyDefinitionYaml::parse_str(yaml).unwrap();
    assert_eq!(definition.version().as_str(), "2.0");
    assert!(definition
        .guards()
        .keys()
        .any(|guard| guard.as_str() == "release_approved"));
    assert!(definition
        .transitions()
        .iter()
        .any(|transition| transition.trigger().as_str() == "publish_outcome"));
    for step in definition.steps().values() {
        assert_eq!(step.handler_kind().as_str(), "host_callback");
    }
    assert!(yaml.contains("step_default: 321"), "{yaml}");
    assert!(yaml.contains("max_attempts: 4"), "{yaml}");
    assert!(yaml.contains("backoff_seconds: 3"), "{yaml}");
    assert!(yaml.contains("see_prior: false"), "{yaml}");
    assert!(yaml.contains("num_agents: 2"), "{yaml}");
    assert!(yaml.contains("rounds: 1"), "{yaml}");
    assert!(
        yaml.contains("output_field:draft_options:decision=key="),
        "{yaml}"
    );
    assert!(
        yaml.contains("step_repeat_exhausted:draft_options"),
        "{yaml}"
    );
}

async fn assert_opening_contexts(arms: &ParityArms) {
    for (index, ceremony_id) in [PUBLISHED_SESSION_ID, ONE_SHOT_ID].iter().enumerate() {
        let instance = checked(
            arms,
            1_100 + 2 * index as u64,
            "made_get_ceremony_instance",
            json!({"ceremony_id": ceremony_id}),
        )
        .await;
        assert_eq!(structured(&instance)["context"], opening_context());
        let events = checked(
            arms,
            1_101 + 2 * index as u64,
            "made_read_ceremony_events",
            json!({"ceremony_id": ceremony_id}),
        )
        .await;
        let first = &structured(&events)["records"][0];
        assert_eq!(first["actor"]["kind"], "engine");
        assert_eq!(first["event"]["context"], opening_context());
        if *ceremony_id == ONE_SHOT_ID {
            assert_lease(structured(&events), "work", "parity-host", None, 17_000);
        }
    }
}

async fn assert_reason_variants(arms: &ParityArms) {
    let references = [
        json!({"kind": "step", "step_id": "work"}),
        json!({"kind": "agenda_item", "agenda_item": "what-happened"}),
        json!({"kind": "contribution", "agenda_item": "what-happened", "ordinal": 0}),
        json!({"kind": "guard_decision", "guard_name": "human_approved"}),
        json!({"kind": "transition", "ordinal": 1}),
    ];
    let kinds = [
        "authorizes",
        "chosen_because",
        "achieved_by",
        "follows_from",
        "satisfies_constraint",
        "violates_constraint",
        "supersedes",
        "contradicts",
    ];
    let confidences = ["high", "medium", "low"];
    let actors = ["human", "agent", "service", "engine"];
    for (index, kind) in kinds.iter().enumerate() {
        // The first three kinds are testimony: OBSERVER authored this contribution.
        let from = if index < 3 {
            &references[2]
        } else {
            &references[index % references.len()]
        };
        let mut to = &references[(index + 1) % references.len()];
        if from == to {
            to = &references[(index + 2) % references.len()];
        }
        let why = format!("Parity evidence for {kind}");
        let confidence = confidences[index % confidences.len()];
        let answer = checked(
            arms,
            1_200 + index as u64,
            "made_assert_ceremony_reason",
            json!({
                "ceremony_id": SESSION_ID, "role_id": "OBSERVER",
                "role_kind": actors[index % actors.len()], "from": from, "to": to,
                "kind": kind, "why": why, "confidence": confidence,
            }),
        )
        .await;
        let reason = structured(&answer)["reasons"]
            .as_array()
            .unwrap()
            .iter()
            .find(|reason| reason["why"] == why)
            .expect("the asserted reason is returned");
        assert_eq!(reason["kind"], *kind);
        assert_eq!(reason["confidence"], confidence);
        assert_eq!(reason["asserted_by_role_id"], "OBSERVER");
        for (expected, actual) in [(from, &reason["from"]), (to, &reason["to"])] {
            for (key, value) in expected.as_object().unwrap() {
                assert_eq!(&actual[key], value);
            }
        }
    }
    // Hashes, trace and actor kinds cross the same unchanged record renderer.
    let events = checked(
        arms,
        1_250,
        "made_read_ceremony_events",
        json!({"ceremony_id": SESSION_ID}),
    )
    .await;
    assert_lease(
        structured(&events),
        "work",
        "parity-host",
        Some("parity-work-1"),
        19_000,
    );
    let verdict = checked(
        arms,
        1_251,
        "made_verify_ceremony_journal",
        json!({"ceremony_id": SESSION_ID}),
    )
    .await;
    assert_eq!(structured(&verdict)["intact"], true);
}

async fn assert_completion_variants(arms: &ParityArms) {
    let statuses = ["completed", "failed", "waiting_for_human", "cancelled"];
    let actors = ["human", "agent", "service", "engine"];
    for (index, status) in statuses.iter().enumerate() {
        let id = 1_300 + 10 * index as u64;
        let ceremony_id = format!("parity-result-{status}");
        checked(arms, id, "made_start_ceremony", json!({
            "ceremony_id": ceremony_id, "definition_yaml": PUBLISHED_CEREMONY,
            "actor_id": "parity-operator", "actor_kind": actors[index], "context": opening_context()
        })).await;
        checked(arms, id + 1, "made_claim_ceremony_step", json!({
            "ceremony_id": ceremony_id, "step_id": "work", "actor_kind": actors[index],
            "lease_owner_id": "parity-results-host", "idempotency_key": format!("claim-{status}"),
            "lease_ttl_ms": 23_000,
        })).await;
        let output =
            json!({"observed_status": status, "artifact": {"uri": "artifact:result", "count": 2}});
        let mut complete = json!({"ceremony_id": ceremony_id, "step_id": "work",
            "actor_kind": actors[index], "status": status, "output": output});
        if *status == "failed" {
            complete["error"] = json!("The provider refused the request.");
        }
        let result = checked(arms, id + 2, "made_complete_ceremony_step", complete).await;
        let step = &structured(&result)["steps"][0];
        assert_eq!(step["status"], *status);
        assert_eq!(step["output"], output);
        assert_eq!(
            step["error"],
            if *status == "failed" {
                json!("The provider refused the request.")
            } else {
                Value::Null
            }
        );
        let events = checked(
            arms,
            id + 3,
            "made_read_ceremony_events",
            json!({"ceremony_id": ceremony_id}),
        )
        .await;
        assert_lease(
            structured(&events),
            "work",
            "parity-results-host",
            Some(&format!("claim-{status}")),
            23_000,
        );
        let verdict = checked(
            arms,
            id + 4,
            "made_verify_ceremony_journal",
            json!({"ceremony_id": ceremony_id}),
        )
        .await;
        assert_eq!(structured(&verdict)["intact"], true);
    }
    let metrics = checked(arms, 1_400, "made_get_metrics", json!({})).await;
    let family = structured(&metrics)["registry"]
        .as_array()
        .unwrap()
        .iter()
        .find(|family| family["name"] == "made_ceremony_step_total")
        .unwrap();
    for status in statuses {
        let sample = family["samples"]
            .as_array()
            .unwrap()
            .iter()
            .find(|sample| {
                sample["labels"]["ceremony"] == "parity_published"
                    && sample["labels"]["step"] == "work"
                    && sample["labels"]["status"] == status
            })
            .unwrap_or_else(|| panic!("missing metric for {status}: {family}"));
        assert_eq!(sample["value"], 1);
    }
}

fn assert_lease(events: &Value, step: &str, owner: &str, key: Option<&str>, ttl_ms: i128) {
    let lease = events["records"]
        .as_array()
        .unwrap()
        .iter()
        .map(|record| &record["event"])
        .find(|event| event["step_id"] == step && event.get("lease").is_some())
        .expect("a sealed start event carries the requested lease")["lease"]
        .clone();
    assert_eq!(lease["owner_id"], owner);
    if let Some(key) = key {
        assert_eq!(lease["idempotency_key"], key);
    }
    let acquired = time::OffsetDateTime::parse(
        lease["acquired_at"].as_str().unwrap(),
        &time::format_description::well_known::Rfc3339,
    )
    .unwrap();
    let expires = time::OffsetDateTime::parse(
        lease["expires_at"].as_str().unwrap(),
        &time::format_description::well_known::Rfc3339,
    )
    .unwrap();
    assert_eq!((expires - acquired).whole_milliseconds(), ttl_ms);
}

#[test]
fn status_text_normalisation_preserves_every_non_version_value() {
    let answer = |version: &str, health: &str, uptime: u64| {
        json!({
            "content": [{"type": "text", "text": json!({"version": version, "health": health,
                "uptime_seconds": uptime, "stats": {"total_orchestrations": 7}}).to_string()}],
            "structuredContent": {"version": version, "health": "healthy", "uptime_seconds": 11,
                "stats": {"total_orchestrations": 7}}
        })
    };
    let first = normalise(&answer("embedded", "healthy", 11), "", "made_get_status");
    assert_eq!(
        first,
        normalise(&answer("grpc", "healthy", 11), "", "made_get_status")
    );
    assert_ne!(
        first,
        normalise(&answer("grpc", "degraded", 11), "", "made_get_status")
    );
    assert_ne!(
        first,
        normalise(&answer("grpc", "healthy", 12), "", "made_get_status")
    );
}
