//! The published catalogue, read back: proto → JSON.
//!
//! The same keys the in-process backend writes, with the same rule for
//! absence: an empty description or cursor on the wire is `null` here,
//! because proto3 cannot tell "none" from "empty" and a reader should
//! not have to.

use made_mcp_proto::v1 as pb;
use serde_json::{json, Value};

pub(crate) fn list_ceremony_definitions_to_json(
    response: pb::ListCeremonyDefinitionsResponse,
) -> Value {
    json!({
        "definitions": response
            .definitions
            .into_iter()
            .map(summary_to_json)
            .collect::<Vec<_>>(),
        "next_cursor": absent_if_empty(response.next_cursor),
    })
}

pub(crate) fn get_ceremony_definition_to_json(
    response: &pb::GetCeremonyDefinitionResponse,
) -> Value {
    json!({
        "ceremony": response.ceremony,
        "version": response.version,
        "digest": response.digest,
        "definition_yaml": response.definition_yaml,
    })
}

/// An unreadable entry has no description or counts to give, and says
/// so with `null` rather than with zeros that would read as a real
/// definition with no states.
fn summary_to_json(summary: pb::PublishedCeremonyDefinitionSummary) -> Value {
    let readable = summary.unreadable.is_empty();
    json!({
        "ceremony": summary.ceremony,
        "version": summary.version,
        "digest": absent_if_empty(summary.digest),
        "description": absent_if_empty(summary.description),
        "state_count": readable.then_some(summary.state_count),
        "step_count": readable.then_some(summary.step_count),
        "unreadable": absent_if_empty(summary.unreadable),
    })
}

fn absent_if_empty(value: String) -> Value {
    if value.is_empty() {
        Value::Null
    } else {
        Value::String(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_wire_fields_read_as_absent() {
        let listing = list_ceremony_definitions_to_json(pb::ListCeremonyDefinitionsResponse {
            definitions: vec![pb::PublishedCeremonyDefinitionSummary {
                ceremony: "review".to_owned(),
                version: "1.0".to_owned(),
                digest: "ab".to_owned(),
                description: String::new(),
                state_count: 2,
                step_count: 1,
                unreadable: String::new(),
            }],
            next_cursor: String::new(),
        });

        assert_eq!(listing["next_cursor"], Value::Null);
        assert_eq!(listing["definitions"][0]["description"], Value::Null);
        assert_eq!(listing["definitions"][0]["state_count"], 2);
        assert_eq!(listing["definitions"][0]["unreadable"], Value::Null);
    }

    #[test]
    fn an_unreadable_entry_has_no_counts_to_give() {
        let listing = list_ceremony_definitions_to_json(pb::ListCeremonyDefinitionsResponse {
            definitions: vec![pb::PublishedCeremonyDefinitionSummary {
                ceremony: "damaged".to_owned(),
                version: "1.0".to_owned(),
                digest: String::new(),
                description: String::new(),
                state_count: 0,
                step_count: 0,
                unreadable: "the stored publication digest does not match".to_owned(),
            }],
            next_cursor: String::new(),
        });

        let entry = &listing["definitions"][0];
        assert_eq!(entry["state_count"], Value::Null);
        assert_eq!(entry["digest"], Value::Null);
        assert!(entry["unreadable"].as_str().unwrap().contains("digest"));
    }
}
