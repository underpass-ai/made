//! How a number a caller writes is read, before anything is sealed.
//!
//! `google.protobuf.Struct` carries every number as a double. A caller
//! who sends `{"severity": 2}` hands the server `2.0`, and a caller who
//! sends `{"score": 1.0}` hands it the same eight bytes: the two are the
//! same value on the wire and cannot be told apart again. In process
//! nothing ever leaves JSON, so both survive as written.
//!
//! That left the sealed record of one session depending on which engine
//! answered — `{"score":1.0}` in process, `{"score":1}` over the wire,
//! two digests, one ceremony — and a client verifying the chain it was
//! handed could fail on an intact one. The `Struct` cannot carry the
//! distinction, so nothing downstream can recover it; the only place it
//! can be settled is before the call reaches either engine.
//!
//! So it is settled here, once, for both arms: **a whole-valued number
//! is read whole, and a number outside ±2^53 is refused**, because past
//! that a double no longer counts one at a time and no surface could
//! promise the value back. Everything else — every fraction — is left
//! exactly as the caller wrote it.
//!
//! The rule is data as well as code: `docs/architecture/struct-numbers.tsv`
//! is the table, and the test at the bottom of this file and the one in
//! `made-adapters`' `attributes.rs` — the gRPC server's own ingress, for
//! clients that never pass through here — are pinned against the same
//! rows.
//!
//! Carrying the exact bytes on the wire instead is the other answer, and
//! it is a phase-2 contract change: it means a field beside every
//! `Struct` that says which of its numbers were written whole, on seven
//! messages. Deferred, deliberately, and written down in the changelog
//! rather than left implied.

use serde_json::{Number, Value};

use super::tool_error::ToolError;

/// Largest whole number a double still counts one at a time: 2^53.
const EXACT_WHOLE_LIMIT: f64 = 9_007_199_254_740_992.0;

/// The same, as an integer, for a value that arrived as one.
const EXACT_WHOLE_LIMIT_INTEGER: i128 = 9_007_199_254_740_992;

/// The rule in the words a tool schema uses, so a caller reads it where
/// they write the value.
pub(crate) const STRUCT_NUMBER_RULE: &str =
    "Numbers in this object are read at ingress on every backend: a whole-valued \
     number is read whole, so `1.0` and `1` are stored and sealed identically, and \
     a number outside ±2^53 is refused — past that a double no longer counts one \
     at a time, and the contract carries this object as `google.protobuf.Struct`.";

/// Every number in the arguments, read by the rule above.
///
/// The whole tree, not a list of fields: the open payloads are where
/// this matters, and a list of which ones they are would be one more
/// thing to keep in step with the catalog. A declared integer field is
/// unaffected — it is already whole — and a declared float field would
/// be read the same way on both arms, which is the property being
/// bought.
pub(crate) fn normalise_numbers(arguments: &Value) -> Result<Value, ToolError> {
    walk(arguments, "tools/call.arguments")
}

fn walk(value: &Value, path: &str) -> Result<Value, ToolError> {
    match value {
        Value::Number(number) => number_at(number, path),
        Value::Array(items) => items
            .iter()
            .enumerate()
            .map(|(index, item)| walk(item, &format!("{path}[{index}]")))
            .collect::<Result<Vec<_>, _>>()
            .map(Value::Array),
        Value::Object(fields) => fields
            .iter()
            .map(|(key, child)| Ok((key.clone(), walk(child, &format!("{path}.{key}"))?)))
            .collect::<Result<serde_json::Map<_, _>, ToolError>>()
            .map(Value::Object),
        leaf => Ok(leaf.clone()),
    }
}

fn number_at(number: &Number, path: &str) -> Result<Value, ToolError> {
    // An integer the caller wrote as one is already read whole. It is
    // checked against the limit as an integer rather than through a
    // double, because asking a double whether it is too big for a
    // double is a question that answers itself wrongly.
    if let Some(whole) = number.as_i64() {
        return within_limit(i128::from(whole), path).map(|()| Value::Number(number.clone()));
    }
    if let Some(whole) = number.as_u64() {
        return within_limit(i128::from(whole), path).map(|()| Value::Number(number.clone()));
    }

    let value = number
        .as_f64()
        .ok_or_else(|| out_of_range(&number.to_string(), path))?;
    if !value.is_finite() {
        return Err(out_of_range(&number.to_string(), path));
    }
    if value.fract() != 0.0 {
        return Ok(Value::Number(number.clone()));
    }
    if value.abs() > EXACT_WHOLE_LIMIT {
        return Err(out_of_range(&number.to_string(), path));
    }
    #[allow(clippy::cast_possible_truncation)] // guarded above: whole, finite, within 2^53
    let whole = value as i64;
    Ok(Value::Number(Number::from(whole)))
}

fn within_limit(value: i128, path: &str) -> Result<(), ToolError> {
    if value.abs() > EXACT_WHOLE_LIMIT_INTEGER {
        return Err(out_of_range(&value.to_string(), path));
    }
    Ok(())
}

/// One wording, on both arms, because the two arms produce it from the
/// same call in the same place.
fn out_of_range(value: &str, path: &str) -> ToolError {
    ToolError::invalid_request(format!(
        "`{path}` is {value}, outside ±2^53; MADE carries an open payload as \
         `google.protobuf.Struct`, whose numbers are doubles, so a value past \
         that range cannot be handed back as it was written"
    ))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::protocol::ToolErrorCode;

    /// The checked-in table both implementations follow.
    const STRUCT_NUMBERS_TSV: &str =
        include_str!("../../../../docs/architecture/struct-numbers.tsv");

    /// One row: the literal a caller writes, what becomes of it, and
    /// what it is read as.
    struct NumberRow {
        input: String,
        outcome: String,
        read_as: String,
    }

    fn number_rows(tsv: &str) -> Vec<NumberRow> {
        let mut rows = Vec::new();
        for (index, line) in tsv.lines().enumerate() {
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
            assert!(
                matches!(cells[1], "whole" | "kept" | "refused"),
                "struct-numbers.tsv line {number} has the unknown outcome `{}`",
                cells[1]
            );
            rows.push(NumberRow {
                input: cells[0].to_owned(),
                outcome: cells[1].to_owned(),
                read_as: cells[2].to_owned(),
            });
        }
        assert!(rows.len() > 5, "struct-numbers.tsv has no rows to follow");
        rows
    }

    #[test]
    fn every_row_of_the_table_is_read_the_way_the_table_says() {
        for row in number_rows(STRUCT_NUMBERS_TSV) {
            let written: Value = serde_json::from_str(&row.input)
                .unwrap_or_else(|error| panic!("`{}` is not JSON: {error}", row.input));
            let arguments = json!({ "context": { "value": written } });
            let read = normalise_numbers(&arguments);

            if row.outcome == "refused" {
                let Err(error) = read else {
                    panic!("`{}` should be refused", row.input)
                };
                assert_eq!(error.code(), ToolErrorCode::InvalidRequest);
                assert!(
                    error.message().contains("2^53"),
                    "the refusal must say why: {}",
                    error.message()
                );
                assert!(
                    error.message().contains("context.value"),
                    "the refusal must name the field: {}",
                    error.message()
                );
                continue;
            }

            let expected: Value = serde_json::from_str(&row.read_as)
                .unwrap_or_else(|error| panic!("`{}` is not JSON: {error}", row.read_as));
            let read = read
                .unwrap_or_else(|error| panic!("`{}` should be read: {error}", row.input))
                ["context"]["value"]
                .clone();
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

    /// The one case the shared table cannot carry: an integer the
    /// caller wrote exactly, one past the range a double can count.
    ///
    /// The gRPC server never sees it — the wire rounded it down to 2^53
    /// before the server's own ingress ran — which is exactly why it is
    /// refused here, where the exact value is still readable, rather
    /// than accepted and handed back as a different number.
    #[test]
    fn an_integer_one_past_the_range_is_refused_rather_than_rounded() {
        let error = normalise_numbers(&json!({ "context": { "id": 9_007_199_254_740_993_u64 } }))
            .expect_err("a double cannot count this one at a time");
        assert_eq!(error.code(), ToolErrorCode::InvalidRequest);
    }

    /// Nesting is not a hiding place: the rule reaches every number in
    /// the call, whatever it is wrapped in.
    #[test]
    fn the_rule_reaches_a_number_at_any_depth() {
        let read = normalise_numbers(&json!({
            "output": {
                "findings": [{ "score": 1.0, "notes": ["fine"] }],
                "attempts": 2,
                "ratio": 0.5,
            },
        }))
        .expect("nothing here is out of range");

        assert_eq!(
            read,
            json!({
                "output": {
                    "findings": [{ "score": 1, "notes": ["fine"] }],
                    "attempts": 2,
                    "ratio": 0.5,
                },
            })
        );
    }

    /// Strings that look like numbers are strings. The rule is about
    /// what the wire does to numbers, and it does nothing to text.
    #[test]
    fn everything_that_is_not_a_number_is_untouched() {
        let arguments = json!({
            "context": { "written": "1.0", "flag": true, "nothing": null },
        });
        assert_eq!(normalise_numbers(&arguments).unwrap(), arguments);
    }

    #[test]
    fn the_rule_the_schemas_publish_says_what_this_does() {
        assert!(STRUCT_NUMBER_RULE.contains("2^53"));
        assert!(STRUCT_NUMBER_RULE.contains("every backend"));
    }
}
