use super::{optional_pb_struct, optional_str, pb, require_object, require_str, Value};

pub(super) fn optional_external_context(
    value: Option<&Value>,
) -> Result<Option<pb::ExternalContextBundle>, String> {
    let Some(value) = value else { return Ok(None) };
    if value.is_null() {
        return Ok(None);
    }
    let obj = require_object(value, "external_context")?;
    let items = obj
        .get("items")
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .map(context_item_from_json)
                .collect::<Result<Vec<_>, _>>()
        })
        .transpose()?
        .unwrap_or_default();
    let references = obj
        .get("references")
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .map(context_reference_from_json)
                .collect::<Result<Vec<_>, _>>()
        })
        .transpose()?
        .unwrap_or_default();
    Ok(Some(pb::ExternalContextBundle {
        bundle_id: optional_str(obj, "bundle_id").unwrap_or("").to_string(),
        schema_version: optional_str(obj, "schema_version")
            .unwrap_or("")
            .to_string(),
        summary: optional_context_summary(obj.get("summary"))?,
        items,
        references,
        metadata: optional_pb_struct(obj, "metadata")?,
    }))
}

fn optional_context_summary(value: Option<&Value>) -> Result<Option<pb::ContextSummary>, String> {
    let Some(value) = value else { return Ok(None) };
    if value.is_null() {
        return Ok(None);
    }
    let obj = require_object(value, "external_context.summary")?;
    Ok(Some(pb::ContextSummary {
        text: optional_str(obj, "text").unwrap_or("").to_string(),
        attributes: optional_pb_struct(obj, "attributes")?,
    }))
}

fn context_item_from_json(value: &Value) -> Result<pb::ContextItem, String> {
    let obj = require_object(value, "external_context.items[]")?;
    let reference_ids = obj
        .get("reference_ids")
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    Ok(pb::ContextItem {
        item_id: require_str(obj, "item_id")?.to_string(),
        kind: require_str(obj, "kind")?.to_string(),
        title: optional_str(obj, "title").unwrap_or("").to_string(),
        narrative: optional_str(obj, "narrative").unwrap_or("").to_string(),
        attributes: optional_pb_struct(obj, "attributes")?,
        reference_ids,
    })
}

fn context_reference_from_json(value: &Value) -> Result<pb::ContextReference, String> {
    let obj = require_object(value, "external_context.references[]")?;
    Ok(pb::ContextReference {
        reference_id: require_str(obj, "reference_id")?.to_string(),
        uri: require_str(obj, "uri")?.to_string(),
        title: optional_str(obj, "title").unwrap_or("").to_string(),
        media_type: optional_str(obj, "media_type").unwrap_or("").to_string(),
        attributes: optional_pb_struct(obj, "attributes")?,
    })
}

pub(super) fn optional_task_metadata(
    value: Option<&Value>,
) -> Result<Option<pb::TaskMetadata>, String> {
    let Some(value) = value else { return Ok(None) };
    if value.is_null() {
        return Ok(None);
    }
    let obj = require_object(value, "task.metadata")?;
    Ok(Some(pb::TaskMetadata {
        source_event_id: optional_str(obj, "source_event_id")
            .unwrap_or("")
            .to_string(),
        causation_id: optional_str(obj, "causation_id").unwrap_or("").to_string(),
        correlation_id: optional_str(obj, "correlation_id")
            .unwrap_or("")
            .to_string(),
        council_contract_id: optional_str(obj, "council_contract_id")
            .unwrap_or("")
            .to_string(),
        output_contract_id: optional_str(obj, "output_contract_id")
            .unwrap_or("")
            .to_string(),
        execution_profile: optional_pb_struct(obj, "execution_profile")?,
    }))
}
