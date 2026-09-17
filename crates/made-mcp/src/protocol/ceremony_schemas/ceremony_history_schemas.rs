//! What a session left behind: the schemas of the four reads.
//!
//! Grouped as the capability group `ceremony_history` and the handler
//! file behind it are, because they answer one question — what
//! happened — and the report is what one particular caller does with
//! the answer.

use serde_json::{json, Value};

use crate::protocol::schema_primitives::{string_schema, MAX_ID_LIST_ITEMS};

/// Whether a report is stored anywhere. It is not, on either edition:
/// ADR-006 makes a report a projection of persisted state, and nothing
/// writes one.
///
/// One constant read by both presenters rather than a literal in each,
/// because it is a fact about what a report *is* and not about either
/// transport. It is not a field on the wire: an always-false boolean
/// in the contract would suggest a caller could ask for one that is
/// true.
///
/// Gated on the two backends because a build with neither mounts no
/// presenter to read it.
#[cfg(any(feature = "embedded", feature = "grpc"))]
pub(crate) const REPORT_IS_PERSISTED: bool = false;

pub(crate) fn ceremony_report_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["ceremony_ids"],
        "properties": {
            "ceremony_ids": {
                "type": "array",
                "minItems": 1,
                "maxItems": MAX_ID_LIST_ITEMS,
                "uniqueItems": true,
                "items": string_schema("Identifier of a persisted ceremony instance."),
                "description": format!(
                    "One or more ceremony ids, reported in caller order, at most \
                     {MAX_ID_LIST_ITEMS}. Empty lists, duplicates and unknown ids are errors."
                )
            },
            "title": string_schema("Optional report heading. It affects presentation only and is escaped as untrusted Markdown text.")
        }
    })
}

/// What an unasked-for `limit` takes, and the most any read answers
/// with — above which a read is refused rather than cut down.
///
/// The engine owns both — they are
/// `made_app::usecases::ReadCeremonyEventsInput::DEFAULT_LIMIT` and
/// `MAX_LIMIT` — and they are repeated here because this crate builds
/// without `made-app` when only the gRPC backend is compiled in. A
/// test pins the two against each other, so a schema that promised
/// something the engine does not do fails rather than misleads.
pub(crate) const DEFAULT_EVENT_PAGE_LIMIT: usize = 200;
pub(crate) const EVENT_PAGE_LIMIT_CAP: usize = 1000;

pub(crate) fn read_ceremony_events_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["ceremony_id"],
        "properties": {
            "ceremony_id": string_schema("Started ceremony instance id. A ceremony with no stream is not found."),
            "from_version": {
                "type": "integer",
                "minimum": 0,
                "description": "Stream version you have already seen; the answer starts at the record after it. Omitted or 0 reads from the first record. To continue a read, send back the `next_version` of the previous answer. Reading is by position, not by a named cursor the server keeps for you."
            },
            "limit": {
                "type": "integer",
                "minimum": 0,
                "maximum": EVENT_PAGE_LIMIT_CAP,
                "description": format!(
                    "How many records at most. Omitted or 0 takes {DEFAULT_EVENT_PAGE_LIMIT}; \
                     {EVENT_PAGE_LIMIT_CAP} is the cap and asking for more is refused, never \
                     quietly cut down, so a longer stream is read in further calls from \
                     `next_version`."
                )
            }
        }
    })
}

pub(crate) fn verify_ceremony_journal_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["ceremony_id"],
        "properties": {
            "ceremony_id": string_schema("Started ceremony instance id. A ceremony with no stream is not found.")
        }
    })
}

pub(crate) fn get_ceremony_transcript_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["ceremony_id"],
        "properties": {
            "ceremony_id": string_schema("Started ceremony instance id.")
        }
    })
}

/// The two numbers above are the engine's, written twice.
///
/// `made-mcp` builds without `made-app` when only the gRPC backend is
/// compiled in, so the schema cannot read them from where they are
/// decided. This is what keeps the copy honest: a schema that promised
/// a default or a cap the engine does not apply fails here rather than
/// misleading a caller who read it.
///
/// It needs `made-app` to compare against, so it runs wherever that
/// crate is compiled in — the default build, which is what CI's
/// `cargo test -p made-mcp` and the workspace run both use. A
/// grpc-only build compiles the constants but not the comparison; it
/// is the same source file either way, so the drift is caught before
/// such a build could carry it.
#[cfg(all(test, feature = "embedded"))]
mod tests {
    use made_app::usecases::ReadCeremonyEventsInput;

    use super::*;

    #[test]
    fn the_published_page_limits_are_the_ones_the_engine_applies() {
        assert_eq!(
            DEFAULT_EVENT_PAGE_LIMIT,
            ReadCeremonyEventsInput::DEFAULT_LIMIT
        );
        assert_eq!(EVENT_PAGE_LIMIT_CAP, ReadCeremonyEventsInput::MAX_LIMIT);

        let schema = read_ceremony_events_schema();
        assert_eq!(
            schema["properties"]["limit"]["maximum"],
            json!(EVENT_PAGE_LIMIT_CAP)
        );
        assert!(
            schema["properties"]["limit"]["description"]
                .as_str()
                .expect("the limit carries a description")
                .contains(&DEFAULT_EVENT_PAGE_LIMIT.to_string()),
            "the description must name the default it applies"
        );
    }
}
