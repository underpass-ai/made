//! Request mappers for the reads of what a session left behind.

use made_mcp_proto::v1 as pb;
use made_mcp_proto::v1::made_service_client::MadeServiceClient;
use serde_json::Value;
use tonic::codegen::{Body, Bytes, StdError};

use super::super::json_to_proto as j2p;
use super::super::streaming;
use crate::protocol::ToolError;

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

pub(super) fn build_stream_ceremony_request(
    args: &Value,
) -> Result<pb::StreamCeremonyRequest, String> {
    let obj = j2p::require_object(args, "tools/call.arguments")?;
    let wait_timeout_ms = obj
        .contains_key("wait_timeout_ms")
        .then(|| j2p::optional_u32(obj, "wait_timeout_ms"))
        .transpose()?;
    Ok(pb::StreamCeremonyRequest {
        ceremony_id: j2p::require_str(obj, "ceremony_id")?.to_owned(),
        after_sequence: j2p::optional_u64(obj, "after_sequence")?,
        max_events: j2p::optional_u32(obj, "max_events")?,
        wait_timeout_ms,
    })
}

pub(super) async fn stream_ceremony<T>(
    client: &mut MadeServiceClient<T>,
    arguments: &Value,
) -> Result<Value, ToolError>
where
    T: tonic::client::GrpcService<tonic::body::BoxBody>,
    T::Error: Into<StdError>,
    T::ResponseBody: Body<Data = Bytes> + Send + 'static,
    <T::ResponseBody as Body>::Error: Into<StdError> + Send,
{
    let request = build_stream_ceremony_request(arguments).map_err(ToolError::invalid_request)?;
    let response = client.stream_ceremony(request).await?;
    streaming::collect_ceremony_progress(response.into_inner())
        .await
        .map_err(ToolError::refused)
}

pub(super) fn build_pull_ceremony_events_request(
    args: &Value,
) -> Result<pb::PullCeremonyEventsRequest, String> {
    let obj = j2p::require_object(args, "tools/call.arguments")?;
    let acknowledge_through = if obj.contains_key("acknowledge_through") {
        Some(j2p::optional_u64(obj, "acknowledge_through")?)
    } else {
        None
    };
    Ok(pb::PullCeremonyEventsRequest {
        consumer: j2p::require_str(obj, "consumer")?.to_owned(),
        limit: j2p::optional_u32(obj, "limit")?,
        acknowledge_through,
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

pub(super) fn build_verify_ceremony_journal_request(
    args: &Value,
) -> Result<pb::VerifyCeremonyJournalRequest, String> {
    let obj = j2p::require_object(args, "tools/call.arguments")?;
    Ok(pb::VerifyCeremonyJournalRequest {
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
    fn a_verification_asks_about_one_session() {
        let request =
            build_verify_ceremony_journal_request(&json!({ "ceremony_id": "session-1" })).unwrap();

        assert_eq!(request.ceremony_id, "session-1");
    }

    #[test]
    fn a_verification_without_an_id_is_refused() {
        assert!(build_verify_ceremony_journal_request(&json!({})).is_err());
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
