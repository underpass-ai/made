//! Buffered collector for `StreamDeliberation`.
//!
//! MCP stdio is synchronous request/response — there is no
//! progress-notification surface that would let a coding agent
//! consume the stream live. We buffer the entire server stream into
//! a single response, returning every frame in order plus the final
//! winner pulled out of the last `result`-typed frame for caller
//! convenience.

use futures::StreamExt;
use made_mcp_proto::v1 as pb;
use serde_json::{json, Value};
use tonic::Streaming;

use super::proto_to_json::ceremony_event_record_view;
use super::proto_to_json::{deliberation_result_to_json, deliberation_update_to_json};

/// Collect a `StreamDeliberation` server stream into a single JSON
/// response. Errors mid-stream surface as a tool error; partial
/// frames already seen are NOT returned (rejecting half-baked output
/// is honest — the caller can retry).
pub(crate) async fn collect_stream(
    mut stream: Streaming<pb::StreamDeliberationResponse>,
) -> Result<Value, String> {
    let mut frames: Vec<Value> = Vec::new();
    let mut winner: Option<Value> = None;
    let mut task_id: Option<String> = None;

    while let Some(item) = stream.next().await {
        let response =
            item.map_err(|status| format!("stream item failed: {}", status.message()))?;
        let Some(update) = response.update else {
            continue;
        };

        if task_id.is_none() && !update.task_id.is_empty() {
            task_id = Some(update.task_id.clone());
        }

        // Extract the winner from a `result` payload before the
        // value moves into `deliberation_update_to_json`.
        if let Some(pb::deliberation_update::Payload::Result(ref r)) = update.payload {
            winner = Some(deliberation_result_to_json(r.clone()));
        }

        frames.push(deliberation_update_to_json(update));
    }

    Ok(json!({
        "task_id": task_id.unwrap_or_default(),
        "frames": frames,
        "winner": winner.unwrap_or(Value::Null),
    }))
}

/// Collect a bounded ceremony stream. EOF without the required end frame is
/// an error because it carries no trustworthy resume cursor.
pub(crate) async fn collect_ceremony_progress(
    stream: Streaming<pb::StreamCeremonyResponse>,
) -> Result<Value, String> {
    collect_ceremony_progress_items(
        stream
            .map(|item| item.map_err(|status| format!("stream item failed: {}", status.message()))),
    )
    .await
}

async fn collect_ceremony_progress_items<S>(mut stream: S) -> Result<Value, String>
where
    S: futures::Stream<Item = Result<pb::StreamCeremonyResponse, String>> + Unpin,
{
    let mut records = Vec::new();
    let mut ending = None;
    while let Some(item) = stream.next().await {
        let response = item?;
        match response.frame {
            Some(pb::stream_ceremony_response::Frame::Record(record)) if ending.is_none() => {
                records.push(ceremony_event_record_view(record, None).to_json());
            }
            Some(pb::stream_ceremony_response::Frame::End(end)) if ending.is_none() => {
                ending = Some(end);
            }
            Some(_) => return Err("progress stream emitted a frame after its end".to_owned()),
            None => return Err("progress stream emitted an empty frame".to_owned()),
        }
    }
    let end =
        ending.ok_or_else(|| "progress stream closed without its required end frame".to_owned())?;
    let reason = match pb::StreamCeremonyEndReason::try_from(end.reason)
        .unwrap_or(pb::StreamCeremonyEndReason::Unspecified)
    {
        pb::StreamCeremonyEndReason::Terminal => "terminal",
        pb::StreamCeremonyEndReason::EventLimit => "event_limit",
        pb::StreamCeremonyEndReason::WaitElapsed => "wait_elapsed",
        pb::StreamCeremonyEndReason::Unspecified => {
            return Err("progress stream ended without a reason".to_owned())
        }
    };
    Ok(json!({
        "records": records,
        "resume_after_sequence": end.resume_after_sequence,
        "head_sequence": end.head_sequence,
        "end_reason": reason,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn ceremony_progress_eof_without_end_is_an_error() {
        let stream = futures::stream::empty::<Result<pb::StreamCeremonyResponse, String>>();

        let error = collect_ceremony_progress_items(stream).await.unwrap_err();

        assert!(error.contains("without its required end frame"));
    }
}
