//! One line naming every field problem of one object.
//!
//! A caller fixing a malformed object should not discover its defects
//! one refusal at a time. When an object carries fields its schema does
//! not declare, lacks fields its schema requires, or both, the gate
//! names all of them together, then the object's declared shape when it
//! has one.

use serde_json::{Map, Value};

use super::claimed_branch::with_shape_hint;

/// The complaint for `fields` against `schema`, or `None` when every
/// required field is present and no undeclared one is carried.
pub(super) fn field_complaint(
    fields: &Map<String, Value>,
    schema: &Value,
    path: &str,
) -> Option<String> {
    let missing = schema
        .get("required")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .filter(|field| !fields.contains_key(*field))
        .collect::<Vec<_>>();
    let closed = schema.get("additionalProperties") == Some(&Value::Bool(false));
    let properties = schema.get("properties").and_then(Value::as_object);
    let unknown = fields
        .keys()
        .map(String::as_str)
        .filter(|field| closed && !properties.is_some_and(|declared| declared.contains_key(*field)))
        .collect::<Vec<_>>();
    let mut parts = Vec::new();
    if !unknown.is_empty() {
        parts.push(listed("unknown field", &unknown));
    }
    if !missing.is_empty() {
        parts.push(listed("missing required field", &missing));
    }
    if parts.is_empty() {
        return None;
    }
    Some(with_shape_hint(
        schema,
        format!("`{path}`: {}", parts.join("; ")),
    ))
}

fn listed(label: &str, names: &[&str]) -> String {
    let quoted = names
        .iter()
        .map(|name| format!("\"{name}\""))
        .collect::<Vec<_>>()
        .join(", ");
    if names.len() == 1 {
        format!("{label} {quoted}")
    } else {
        format!("{label}s {quoted}")
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn complaint(value: &Value, schema: &Value) -> Option<String> {
        field_complaint(value.as_object().unwrap(), schema, "x")
    }

    #[test]
    fn every_unknown_and_missing_field_is_named_in_one_line() {
        let schema = json!({
            "additionalProperties": false,
            "required": ["a", "b"],
            "properties": { "a": {}, "b": {} },
            "x-made-shape": "use {a, b}"
        });
        assert_eq!(
            complaint(&json!({ "c": 1, "d": 2 }), &schema).unwrap(),
            "`x`: unknown fields \"c\", \"d\"; missing required fields \"a\", \"b\"; use {a, b}"
        );
        assert_eq!(
            complaint(&json!({ "a": 1, "c": 1 }), &schema).unwrap(),
            "`x`: unknown field \"c\"; missing required field \"b\"; use {a, b}"
        );
        assert_eq!(complaint(&json!({ "a": 1, "b": 2 }), &schema), None);
    }

    #[test]
    fn an_open_object_names_only_what_is_missing() {
        let schema = json!({ "required": ["a"] });
        assert_eq!(
            complaint(&json!({ "z": 1 }), &schema).unwrap(),
            "`x`: missing required field \"a\""
        );
    }
}
