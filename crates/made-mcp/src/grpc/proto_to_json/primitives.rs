use made_mcp_proto::v1 as pb;
use prost_types::{
    value::Kind as PbKind, ListValue, Struct as PbStruct, Timestamp, Value as PbValue,
};
use serde_json::{Map, Number as JsonNumber, Value};

pub(crate) fn pb_value_to_json(value: PbValue) -> Value {
    match value.kind {
        None | Some(PbKind::NullValue(_)) => Value::Null,
        Some(PbKind::BoolValue(value)) => Value::Bool(value),
        Some(PbKind::NumberValue(value)) => {
            JsonNumber::from_f64(value).map_or(Value::Null, Value::Number)
        }
        Some(PbKind::StringValue(value)) => Value::String(value),
        Some(PbKind::ListValue(ListValue { values })) => {
            Value::Array(values.into_iter().map(pb_value_to_json).collect())
        }
        Some(PbKind::StructValue(value)) => Value::Object(pb_struct_to_json(value)),
    }
}

pub(crate) fn pb_struct_to_json(value: PbStruct) -> Map<String, Value> {
    value
        .fields
        .into_iter()
        .map(|(key, value)| (key, pb_value_to_json(value)))
        .collect()
}

pub(crate) fn optional_pb_struct_to_json(value: Option<PbStruct>) -> Value {
    value.map_or_else(
        || Value::Object(Map::new()),
        |value| Value::Object(pb_struct_to_json(value)),
    )
}

pub(crate) fn timestamp_to_rfc3339(timestamp: Option<&Timestamp>) -> Value {
    let Some(Timestamp { seconds, nanos }) = timestamp else {
        return Value::Null;
    };
    let nanos_total = i128::from(*seconds) * 1_000_000_000 + i128::from(*nanos);
    time::OffsetDateTime::from_unix_timestamp_nanos(nanos_total)
        .ok()
        .and_then(|instant| {
            instant
                .format(&time::format_description::well_known::Rfc3339)
                .ok()
        })
        .map_or(Value::Null, Value::String)
}

pub(super) fn phase_name(phase: i32) -> &'static str {
    match pb::DeliberationPhase::try_from(phase).unwrap_or(pb::DeliberationPhase::Unspecified) {
        pb::DeliberationPhase::Unspecified => "DELIBERATION_PHASE_UNSPECIFIED",
        pb::DeliberationPhase::Proposing => "DELIBERATION_PHASE_PROPOSING",
        pb::DeliberationPhase::Revising => "DELIBERATION_PHASE_REVISING",
        pb::DeliberationPhase::Validating => "DELIBERATION_PHASE_VALIDATING",
        pb::DeliberationPhase::Scoring => "DELIBERATION_PHASE_SCORING",
        pb::DeliberationPhase::Completed => "DELIBERATION_PHASE_COMPLETED",
    }
}
