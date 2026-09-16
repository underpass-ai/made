//! Request mappers for the three reads of what a session left behind.

use made_mcp_proto::v1 as pb;
use serde_json::Value;

use super::super::json_to_proto as j2p;

pub(super) fn build_read_ceremony_events_request(
    args: &Value,
) -> Result<pb::ReadCeremonyEventsRequest, String> {
    let obj = j2p::require_object(args, "tools/call.arguments")?;
    Ok(pb::ReadCeremonyEventsRequest {
        ceremony_id: j2p::require_str(obj, "ceremony_id")?.to_owned(),
        // Zero says "from the beginning" and "no limit asked for" on
        // the wire, which is what the schema's own default says too.
        from_version: j2p::optional_u64(obj, "from_version")?,
        limit: j2p::optional_u32(obj, "limit")?,
    })
}

pub(super) fn build_get_ceremony_transcript_request(
    args: &Value,
) -> Result<pb::GetCeremonyTranscriptRequest, String> {
    let obj = j2p::require_object(args, "tools/call.arguments")?;
    Ok(pb::GetCeremonyTranscriptRequest {
        ceremony_id: j2p::require_str(obj, "ceremony_id")?.to_owned(),
    })
}

pub(super) fn build_generate_ceremony_report_request(
    args: &Value,
) -> Result<pb::GenerateCeremonyReportRequest, String> {
    let obj = j2p::require_object(args, "tools/call.arguments")?;
    Ok(pb::GenerateCeremonyReportRequest {
        ceremony_ids: j2p::string_array(obj, "ceremony_ids"),
        // An omitted title is an empty one here; the engine takes its
        // own default, and the schema refuses a blank one before this
        // mapper ever runs.
        title: j2p::optional_str(obj, "title")
            .unwrap_or_default()
            .to_owned(),
    })
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn a_read_with_only_an_id_asks_for_the_whole_stream_with_the_server_default() {
        let request =
            build_read_ceremony_events_request(&json!({ "ceremony_id": "session-1" })).unwrap();

        assert_eq!(request.ceremony_id, "session-1");
        assert_eq!(request.from_version, 0);
        assert_eq!(request.limit, 0);
    }

    #[test]
    fn a_read_carries_the_version_and_the_limit_it_was_given() {
        let request = build_read_ceremony_events_request(&json!({
            "ceremony_id": "session-1",
            "from_version": 7,
            "limit": 3,
        }))
        .unwrap();

        assert_eq!(request.from_version, 7);
        assert_eq!(request.limit, 3);
    }

    #[test]
    fn a_report_without_a_title_sends_an_empty_one() {
        let request =
            build_generate_ceremony_report_request(&json!({ "ceremony_ids": ["a", "b"] })).unwrap();

        assert_eq!(request.ceremony_ids, ["a", "b"]);
        assert!(request.title.is_empty());
    }

    #[test]
    fn a_report_carries_the_title_it_was_given() {
        let request = build_generate_ceremony_report_request(
            &json!({ "ceremony_ids": ["a"], "title": "Session review" }),
        )
        .unwrap();

        assert_eq!(request.title, "Session review");
    }
}
