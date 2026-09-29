//! Which `oneOf` alternative a refused value was written for.
//!
//! "Satisfies 0 of the 3 mutually exclusive alternatives" is true and
//! useless: a caller who wrote a group stage with a malformed `repeat`
//! needs the complaint about that `repeat`, not a list of what a plain
//! stage would have required. An alternative *claims* a value when the
//! value carries every field the alternative requires and agrees with
//! each `const` or `enum` discriminator the alternative declares for a
//! field it carries. When exactly one alternative claims the value, the
//! gate reports that alternative's own complaint; otherwise the value's
//! intent is ambiguous and the generic sentence stands.

use serde_json::Value;

/// Schema keyword holding the expected shape of an object, appended to
/// a complaint about that object's fields so the caller learns the form
/// the tool accepts instead of only what it refused.
pub(super) const SHAPE_HINT: &str = "x-made-shape";

/// The single alternative a value was evidently written for, if any.
pub(super) fn claimed_branch<'a>(value: &Value, branches: &'a [Value]) -> Option<&'a Value> {
    let fields = value.as_object()?;
    let mut claimants = branches.iter().filter(|branch| {
        let required_present = branch
            .get("required")
            .and_then(Value::as_array)
            .is_some_and(|required| {
                !required.is_empty()
                    && required
                        .iter()
                        .filter_map(Value::as_str)
                        .all(|field| fields.contains_key(field))
            });
        required_present
            && branch
                .get("properties")
                .and_then(Value::as_object)
                .into_iter()
                .flatten()
                .filter_map(|(field, property)| Some((fields.get(field)?, property)))
                .all(|(actual, property)| discriminator_agrees(actual, property))
    });
    let claimant = claimants.next()?;
    claimants.next().is_none().then_some(claimant)
}

fn discriminator_agrees(actual: &Value, property: &Value) -> bool {
    if let Some(expected) = property.get("const") {
        return actual == expected;
    }
    property
        .get("enum")
        .and_then(Value::as_array)
        .is_none_or(|variants| variants.contains(actual))
}

/// `complaint`, followed by the object's declared shape when it has one.
pub(super) fn with_shape_hint(schema: &Value, complaint: String) -> String {
    match schema.get(SHAPE_HINT).and_then(Value::as_str) {
        Some(hint) => format!("{complaint}; {hint}"),
        None => complaint,
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn a_value_carrying_one_alternative_s_required_fields_claims_it() {
        let branches = [
            json!({ "required": ["id", "owner"] }),
            json!({ "required": ["id", "group"] }),
        ];
        let value = json!({ "id": "x", "group": {} });
        assert_eq!(claimed_branch(&value, &branches), Some(&branches[1]));
    }

    #[test]
    fn a_discriminator_that_disagrees_withdraws_the_claim() {
        let branches = [
            json!({ "required": ["kind", "step", "output_field"],
                    "properties": { "kind": { "const": "output_field" } } }),
            json!({ "required": ["kind", "step"],
                    "properties": { "kind": { "const": "step_repeat_exhausted" } } }),
            json!({ "required": ["kind", "step"],
                    "properties": { "kind": { "enum": ["a", "b"] } } }),
        ];
        let value = json!({ "kind": "output_field", "step": "s", "output_field": "f" });
        assert_eq!(claimed_branch(&value, &branches), Some(&branches[0]));
    }

    #[test]
    fn ambiguity_or_no_claimant_leaves_the_generic_complaint() {
        let branches = [json!({ "required": ["a"] }), json!({ "required": ["b"] })];
        assert_eq!(claimed_branch(&json!({ "a": 1, "b": 2 }), &branches), None);
        assert_eq!(claimed_branch(&json!({ "c": 1 }), &branches), None);
        assert_eq!(claimed_branch(&json!("text"), &branches), None);
        assert_eq!(
            claimed_branch(&json!({}), &[json!({ "required": [] })]),
            None,
            "an alternative that requires nothing claims nothing"
        );
    }

    #[test]
    fn a_shape_hint_is_appended_only_where_declared() {
        let plain = with_shape_hint(&json!({}), "refused".to_owned());
        assert_eq!(plain, "refused");
        let hinted = with_shape_hint(&json!({ SHAPE_HINT: "use {a}" }), "refused".to_owned());
        assert_eq!(hinted, "refused; use {a}");
    }
}
