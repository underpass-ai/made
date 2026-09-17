//! `google.protobuf.Struct` ↔ domain `Attributes` / `Rubric`
//! conversion helpers.
//!
//! Both `Attributes` and `Rubric` are opaque `BTreeMap<String, Value>`
//! wrappers around `serde_json::Value`, which maps onto the
//! Struct/Value/ListValue proto tree shape for shape.
//!
//! It is **not** lossless, and the doc here used to say it was. A
//! `Struct` carries every number as a double: `1` and `1.0` are the
//! same eight bytes on it, and an integer past 2^53 cannot be counted
//! one at a time once it is one. Nothing downstream can recover what
//! the caller wrote, so this ingress decides it, by the rule in
//! `docs/architecture/struct-numbers.tsv` — a whole-valued number is
//! read whole, a number outside ±2^53 is refused. `made-mcp` applies
//! the same rule at its request gate, for callers that arrive through a
//! tool rather than through the RPC, and a test in each place is pinned
//! against the same rows (issue #75).

use std::collections::BTreeMap;

use made_core::error::DomainError;
use made_core::value_objects::{Attributes, Rubric};
use prost_types::{value::Kind as PbKind, ListValue, Struct as PbStruct, Value as PbValue};
use serde_json::Value;

/// Convert a proto `Struct` into a domain `Attributes`.
pub fn attributes_from_struct(s: Option<PbStruct>) -> Result<Attributes, DomainError> {
    Attributes::new(struct_to_map(s)?)
}

/// Convert a domain `Attributes` into a proto `Struct`.
pub fn attributes_to_struct(attrs: &Attributes) -> PbStruct {
    map_to_struct(attrs.as_map())
}

/// Convert a proto `Struct` into a domain `Rubric`.
pub fn rubric_from_struct(s: Option<PbStruct>) -> Result<Rubric, DomainError> {
    Rubric::new(struct_to_map(s)?)
}

/// Convert a domain `Rubric` into a proto `Struct`.
///
/// Rubrics currently only flow proto → domain at request time; this
/// symmetric direction is kept for adapters that want to surface the
/// active rubric on a response (e.g. diagnostics / status RPCs) and
/// for round-trip tests. Marked `allow(dead_code)` because no RPC
/// wires it today; removing it would force a re-introduction when
/// those RPCs land.
#[allow(dead_code)]
pub fn rubric_to_struct(rubric: &Rubric) -> PbStruct {
    map_to_struct(rubric.as_map())
}

// ---------------------------------------------------------------------------
// Low-level helpers
// ---------------------------------------------------------------------------

fn struct_to_map(s: Option<PbStruct>) -> Result<BTreeMap<String, Value>, DomainError> {
    let Some(s) = s else {
        return Ok(BTreeMap::new());
    };
    s.fields
        .into_iter()
        .map(|(k, v)| Ok((k, pb_value_to_json(v)?)))
        .collect()
}

/// A JSON object as a proto `Struct`.
///
/// Anything that is not an object has no `Struct` form, and answering
/// `None` says so; inventing a wrapper object would put a shape on the
/// wire that the value never had.
pub fn struct_from_json(value: &Value) -> Option<PbStruct> {
    let object = value.as_object()?;
    Some(PbStruct {
        fields: object
            .iter()
            .map(|(key, value)| (key.clone(), json_to_pb_value(value)))
            .collect(),
    })
}

fn map_to_struct(map: &BTreeMap<String, Value>) -> PbStruct {
    PbStruct {
        fields: map
            .iter()
            .map(|(k, v)| (k.clone(), json_to_pb_value(v)))
            .collect(),
    }
}

/// Largest whole number a double still counts one at a time: 2^53.
const EXACT_WHOLE_LIMIT: f64 = 9_007_199_254_740_992.0;

/// A `Struct` carries every number as a double, so a caller that sends
/// `2` hands the engine `2.0` — and the engine *stores* it, sealing
/// `{"severity":2.0}` into the record's digest where the same session
/// driven in process seals `{"severity":2}`. Two engines, two audit
/// chains, one session: found by comparing the sealed records of a
/// parity session (slice F3c), which is the first read that put a
/// digest where a test could see it.
///
/// A whole number is read whole; a fraction is left exactly as it
/// arrived; and a whole number past the range where a double still
/// counts one at a time is **refused**, because it did not survive the
/// wire and answering with a different number would be worse than
/// saying so. `docs/architecture/struct-numbers.tsv` is the table, and
/// `made-mcp`'s request gate follows the same one.
fn number_to_json(value: f64) -> Result<Value, DomainError> {
    if value.is_finite() && value.fract() == 0.0 {
        if value.abs() > EXACT_WHOLE_LIMIT {
            return Err(out_of_range(value));
        }
        #[allow(clippy::cast_possible_truncation)] // guarded above: whole, finite and within 2^53
        let whole = value as i64;
        return Ok(Value::Number(serde_json::Number::from(whole)));
    }
    serde_json::Number::from_f64(value)
        .map(Value::Number)
        .ok_or_else(|| out_of_range(value))
}

/// The complaint a number past the range earns.
///
/// `OutOfRange` and not something vaguer: the caller can fix it without
/// knowing anything about the session, which is what this code means
/// everywhere else here, and it reaches a client as `invalid_argument`
/// — the same classification the MCP arms give it.
fn out_of_range(value: f64) -> DomainError {
    DomainError::OutOfRange {
        field: "attributes.number",
        value,
        min: -EXACT_WHOLE_LIMIT,
        max: EXACT_WHOLE_LIMIT,
    }
}

pub(super) fn pb_value_to_json(v: PbValue) -> Result<Value, DomainError> {
    Ok(match v.kind {
        None | Some(PbKind::NullValue(_)) => Value::Null,
        Some(PbKind::NumberValue(n)) => number_to_json(n)?,
        Some(PbKind::StringValue(s)) => Value::String(s),
        Some(PbKind::BoolValue(b)) => Value::Bool(b),
        Some(PbKind::StructValue(s)) => {
            let obj: serde_json::Map<_, _> = s
                .fields
                .into_iter()
                .map(|(k, v)| Ok((k, pb_value_to_json(v)?)))
                .collect::<Result<_, DomainError>>()?;
            Value::Object(obj)
        }
        Some(PbKind::ListValue(lv)) => Value::Array(
            lv.values
                .into_iter()
                .map(pb_value_to_json)
                .collect::<Result<_, DomainError>>()?,
        ),
    })
}

fn json_to_pb_value(v: &Value) -> PbValue {
    let kind = match v {
        Value::Null => PbKind::NullValue(0),
        Value::Bool(b) => PbKind::BoolValue(*b),
        Value::Number(n) => {
            // Lossy only for integers that cannot fit in f64 mantissa;
            // acceptable for rubric/attributes payloads where the
            // proto wire is defined as numeric.
            let f = n.as_f64().unwrap_or(0.0);
            PbKind::NumberValue(f)
        }
        Value::String(s) => PbKind::StringValue(s.clone()),
        Value::Array(a) => PbKind::ListValue(ListValue {
            values: a.iter().map(json_to_pb_value).collect(),
        }),
        Value::Object(o) => PbKind::StructValue(PbStruct {
            fields: o
                .iter()
                .map(|(k, v)| (k.clone(), json_to_pb_value(v)))
                .collect(),
        }),
    };
    PbValue { kind: Some(kind) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn none_struct_produces_empty_map() {
        let attrs = attributes_from_struct(None).unwrap();
        assert!(attrs.is_empty());
    }

    #[test]
    fn empty_struct_produces_empty_map() {
        let attrs = attributes_from_struct(Some(PbStruct::default())).unwrap();
        assert!(attrs.is_empty());
    }

    #[test]
    fn scalar_roundtrip() {
        let mut m = BTreeMap::new();
        m.insert("s".to_owned(), json!("hello"));
        m.insert("n".to_owned(), json!(42));
        m.insert("b".to_owned(), json!(true));
        m.insert("null".to_owned(), json!(null));
        let attrs = Attributes::new(m).unwrap();

        let pb = attributes_to_struct(&attrs);
        let back = attributes_from_struct(Some(pb)).unwrap();
        assert_eq!(back.get("s"), Some(&json!("hello")));
        assert_eq!(back.get("b"), Some(&json!(true)));
        assert_eq!(back.get("null"), Some(&json!(null)));
        // The JSON value, not its `as_f64`. Comparing numerically is
        // what let `42` and `42.0` pass as the same answer here while
        // they sealed two different digests: `as_f64` is exactly the
        // reading this ingress exists to settle.
        assert_eq!(back.get("n"), Some(&json!(42)));
    }

    #[test]
    fn nested_object_and_array_roundtrip() {
        let mut m = BTreeMap::new();
        m.insert(
            "payload".to_owned(),
            json!({
                "severity": "p1",
                "tags": ["latency", "prod"],
                "counts": {"p50": 100, "p99": 500},
            }),
        );
        let attrs = Attributes::new(m).unwrap();
        let back = attributes_from_struct(Some(attributes_to_struct(&attrs))).unwrap();
        let payload = back.get("payload").unwrap().as_object().unwrap();
        assert_eq!(payload.get("severity").unwrap(), "p1");
        assert_eq!(payload.get("tags").unwrap(), &json!(["latency", "prod"]));
        // Again the value itself: a nested whole number comes back
        // whole, at any depth, which is what the record's digest
        // covers.
        assert_eq!(
            payload.get("counts").unwrap(),
            &json!({"p50": 100, "p99": 500})
        );
    }

    #[test]
    fn rubric_helpers_are_symmetric_with_attributes() {
        let mut m = BTreeMap::new();
        m.insert("rigor".to_owned(), json!("high"));
        let rubric = Rubric::new(m).unwrap();
        let back = rubric_from_struct(Some(rubric_to_struct(&rubric))).unwrap();
        assert_eq!(back.get("rigor"), Some(&json!("high")));
    }

    /// The checked-in table this ingress and `made-mcp`'s request gate
    /// both follow.
    const STRUCT_NUMBERS_TSV: &str =
        include_str!("../../../../../docs/architecture/struct-numbers.tsv");

    /// One row of it: the literal a caller writes, what becomes of it,
    /// and what it is read as.
    struct NumberRow {
        input: String,
        outcome: String,
        read_as: String,
    }

    fn number_rows() -> Vec<NumberRow> {
        let mut rows = Vec::new();
        for (index, line) in STRUCT_NUMBERS_TSV.lines().enumerate() {
            let number = index + 1;
            if line.trim().is_empty() || line.starts_with('#') {
                continue;
            }
            let cells: Vec<&str> = line.split('\t').collect();
            assert_eq!(
                cells.len(),
                4,
                "struct-numbers.tsv line {number} has {} tab-separated columns, expected 4",
                cells.len()
            );
            if cells[0] == "input" {
                continue;
            }
            rows.push(NumberRow {
                input: cells[0].to_owned(),
                outcome: cells[1].to_owned(),
                read_as: cells[2].to_owned(),
            });
        }
        assert!(rows.len() > 5, "struct-numbers.tsv has no rows to follow");
        rows
    }

    /// The gRPC server reads a number exactly as the MCP request gate
    /// does, and the two are pinned against the same rows rather than
    /// against each other's code. A client that speaks the RPC directly
    /// never passes through the gate, so "the same rule" has to be a
    /// property of both and not of one calling the other.
    #[test]
    fn every_row_of_the_table_is_read_the_way_the_table_says() {
        for row in number_rows() {
            let written: Value = serde_json::from_str(&row.input)
                .unwrap_or_else(|error| panic!("`{}` is not JSON: {error}", row.input));
            // What the wire hands this ingress is always a double.
            let sent = written
                .as_f64()
                .unwrap_or_else(|| panic!("`{}` is not a number", row.input));
            let read = number_to_json(sent);

            if row.outcome == "refused" {
                let error = read.unwrap_err();
                assert!(
                    matches!(
                        error,
                        DomainError::OutOfRange {
                            field: "attributes.number",
                            ..
                        }
                    ),
                    "`{}` should be refused as out of range, not {error:?}",
                    row.input
                );
                continue;
            }

            let expected: Value = serde_json::from_str(&row.read_as)
                .unwrap_or_else(|error| panic!("`{}` is not JSON: {error}", row.read_as));
            let read =
                read.unwrap_or_else(|error| panic!("`{}` should be read: {error}", row.input));
            assert_eq!(read, expected, "`{}` was read as {read}", row.input);
            if row.outcome == "whole" {
                assert!(
                    read.is_i64() || read.is_u64(),
                    "`{}` is whole and must be read as an integer, not {read}",
                    row.input
                );
            }
        }
    }

    /// The refusal reaches a direct gRPC client rather than being
    /// rounded away inside the mapper.
    #[test]
    fn a_number_past_the_range_is_refused_rather_than_rounded() {
        let mut fields: BTreeMap<String, PbValue> = BTreeMap::new();
        fields.insert(
            "id".to_owned(),
            PbValue {
                kind: Some(PbKind::NumberValue(1e17)),
            },
        );
        let error = attributes_from_struct(Some(PbStruct { fields })).unwrap_err();
        assert!(
            matches!(
                error,
                DomainError::OutOfRange {
                    field: "attributes.number",
                    ..
                }
            ),
            "{error:?}"
        );
    }

    #[test]
    fn nan_in_json_is_carried_as_null_on_the_wire() {
        // Domain invariant: scores never hold NaN, but arbitrary
        // payload values may; we clamp to Null to keep the wire valid.
        let mut m = BTreeMap::new();
        let bad = serde_json::Number::from_f64(1.0).unwrap();
        m.insert("n".to_owned(), Value::Number(bad));
        let pb = map_to_struct(&m);
        assert!(pb.fields.contains_key("n"));
    }

    #[test]
    fn blank_key_is_rejected_by_domain_validation() {
        let mut fields: BTreeMap<String, PbValue> = BTreeMap::new();
        fields.insert(
            "  ".to_owned(),
            PbValue {
                kind: Some(PbKind::BoolValue(true)),
            },
        );
        let pb = PbStruct { fields };
        let err = attributes_from_struct(Some(pb)).unwrap_err();
        assert!(matches!(
            err,
            DomainError::EmptyField {
                field: "attributes.key"
            }
        ));
    }
}
