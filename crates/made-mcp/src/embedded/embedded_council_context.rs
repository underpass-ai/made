use made_core::entities::{
    ContextItem, ContextReference, ContextSummary, ExternalContextBundle, TaskMetadata,
};
use made_core::value_objects::{Attributes, EventId, OutputContractId};
use serde_json::{Map, Value};

pub(super) fn metadata(value: Option<&Value>) -> Result<TaskMetadata, String> {
    let Some(value) = value.filter(|value| !value.is_null()) else {
        return Ok(TaskMetadata::default());
    };
    let value = object(value, "metadata")?;
    Ok(TaskMetadata::new(
        optional_id(value, "source_event_id")?,
        optional_id(value, "causation_id")?,
        optional_id(value, "correlation_id")?,
        optional_string(value, "council_contract_id")
            .filter(|value| !value.trim().is_empty())
            .map(made_core::value_objects::CouncilContractId::new)
            .transpose()
            .map_err(domain)?,
        optional_string(value, "output_contract_id")
            .filter(|value| !value.trim().is_empty())
            .map(OutputContractId::new)
            .transpose()
            .map_err(domain)?,
        attributes(value.get("execution_profile"), "metadata.execution_profile")?,
    ))
}

pub(super) fn external_context(
    value: Option<&Value>,
) -> Result<Option<ExternalContextBundle>, String> {
    let Some(value) = value.filter(|value| !value.is_null()) else {
        return Ok(None);
    };
    let value = object(value, "external_context")?;
    let summary = value
        .get("summary")
        .filter(|value| !value.is_null())
        .map(|summary| {
            let summary = object(summary, "external_context.summary")?;
            ContextSummary::new(
                string(summary, "text")?,
                attributes(
                    summary.get("attributes"),
                    "external_context.summary.attributes",
                )?,
            )
            .map_err(domain)
        })
        .transpose()?;
    let items = array(value, "items")?
        .iter()
        .map(|item| {
            let item = object(item, "external_context.item")?;
            ContextItem::new(
                string(item, "item_id")?,
                string(item, "kind")?,
                string(item, "title")?,
                optional_string(item, "narrative"),
                attributes(item.get("attributes"), "external_context.item.attributes")?,
                strings(item, "reference_ids")?,
            )
            .map_err(domain)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let references = array(value, "references")?
        .iter()
        .map(|reference| {
            let reference = object(reference, "external_context.reference")?;
            ContextReference::new(
                string(reference, "reference_id")?,
                string(reference, "uri")?,
                optional_string(reference, "title"),
                optional_string(reference, "media_type"),
                attributes(
                    reference.get("attributes"),
                    "external_context.reference.attributes",
                )?,
            )
            .map_err(domain)
        })
        .collect::<Result<Vec<_>, _>>()?;
    ExternalContextBundle::new(
        string(value, "bundle_id")?,
        string(value, "schema_version")?,
        summary,
        items,
        references,
        attributes(value.get("metadata"), "external_context.metadata")?,
    )
    .map(Some)
    .map_err(domain)
}

fn object<'a>(value: &'a Value, where_: &str) -> Result<&'a Map<String, Value>, String> {
    value
        .as_object()
        .ok_or_else(|| format!("{where_}: expected an object"))
}

fn string<'a>(object: &'a Map<String, Value>, key: &str) -> Result<&'a str, String> {
    object
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("missing required string `{key}`"))
}

fn optional_string(object: &Map<String, Value>, key: &str) -> Option<String> {
    object.get(key).and_then(Value::as_str).map(str::to_owned)
}

fn attributes(value: Option<&Value>, where_: &str) -> Result<Attributes, String> {
    let values = match value {
        None | Some(Value::Null) => Map::new(),
        Some(value) => object(value, where_)?.clone(),
    };
    Attributes::new(values.into_iter().collect()).map_err(domain)
}

fn array<'a>(object: &'a Map<String, Value>, key: &str) -> Result<&'a [Value], String> {
    match object.get(key) {
        None | Some(Value::Null) => Ok(&[]),
        Some(Value::Array(values)) => Ok(values),
        Some(_) => Err(format!("`{key}` must be an array")),
    }
}

fn strings(object: &Map<String, Value>, key: &str) -> Result<Vec<String>, String> {
    array(object, key)?
        .iter()
        .map(|value| {
            value
                .as_str()
                .map(str::to_owned)
                .ok_or_else(|| format!("`{key}` must contain strings"))
        })
        .collect()
}

fn optional_id(object: &Map<String, Value>, key: &str) -> Result<Option<EventId>, String> {
    optional_string(object, key)
        .filter(|value| !value.trim().is_empty())
        .map(EventId::new)
        .transpose()
        .map_err(domain)
}

fn domain(error: made_core::error::DomainError) -> String {
    let rendered = error.to_string();
    drop(error);
    rendered
}
