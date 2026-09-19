use made_mcp_proto::v1 as pb;
use prost_types::{
    value::Kind as PbKind, ListValue, Struct as PbStruct, Timestamp, Value as PbValue,
};
use serde_json::{Map, Number as JsonNumber, Value};

pub(crate) fn pb_value_to_json(value: PbValue) -> Value {
    match value.kind {
        None | Some(PbKind::NullValue(_)) => Value::Null,
        Some(PbKind::BoolValue(value)) => Value::Bool(value),
        Some(PbKind::NumberValue(value)) => number_to_json(value),
        Some(PbKind::StringValue(value)) => Value::String(value),
        Some(PbKind::ListValue(ListValue { values })) => {
            Value::Array(values.into_iter().map(pb_value_to_json).collect())
        }
        Some(PbKind::StructValue(value)) => Value::Object(pb_struct_to_json(value)),
    }
}

/// Largest whole number a double still counts one at a time: 2^53.
const EXACT_WHOLE_LIMIT: f64 = 9_007_199_254_740_992.0;

/// A `Struct` carries every number as a double, so `2` goes out and
/// `2.0` comes back, while the in-process arm — which never leaves
/// JSON — still says `2`. One context, two readings, decided by which
/// engine served the call. A whole number is answered whole; anything
/// with a fraction, and anything past the range where a double still
/// counts one at a time, is untouched.
fn number_to_json(value: f64) -> Value {
    if value.is_finite() && value.fract() == 0.0 && value.abs() <= EXACT_WHOLE_LIMIT {
        #[allow(clippy::cast_possible_truncation)] // guarded above: whole, finite and within 2^53
        let whole = value as i64;
        return Value::Number(JsonNumber::from(whole));
    }
    JsonNumber::from_f64(value).map_or(Value::Null, Value::Number)
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

pub(crate) fn nullable_pb_struct_to_json(value: Option<PbStruct>) -> Value {
    value.map_or(Value::Null, |value| Value::Object(pb_struct_to_json(value)))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nullable_struct_preserves_absence_instead_of_inventing_an_empty_object() {
        assert_eq!(nullable_pb_struct_to_json(None), Value::Null);
        assert_eq!(
            nullable_pb_struct_to_json(Some(PbStruct::default())),
            serde_json::json!({})
        );
    }

    #[test]
    fn a_whole_double_comes_back_the_way_it_was_sent() {
        assert_eq!(number_to_json(2.0), serde_json::json!(2));
        assert_eq!(number_to_json(-7.0), serde_json::json!(-7));
        assert_eq!(number_to_json(0.0), serde_json::json!(0));
    }

    #[test]
    fn a_fraction_stays_a_fraction() {
        assert_eq!(number_to_json(2.5), serde_json::json!(2.5));
    }

    #[test]
    fn what_a_double_cannot_count_exactly_stays_a_double() {
        let beyond = EXACT_WHOLE_LIMIT * 4.0;
        assert!(number_to_json(beyond).is_f64());
        assert_eq!(number_to_json(f64::NAN), Value::Null);
    }
}
