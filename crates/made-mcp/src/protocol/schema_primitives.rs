use serde_json::{json, Value};

use super::struct_numbers::STRUCT_NUMBER_RULE;

/// The most entries any caller-supplied list of ids carries.
///
/// A bound, and a stated one: every array that declares `uniqueItems`
/// is checked for duplicates, and a list whose length the caller
/// decides is a list somebody can make expensive. A hundred is far more
/// than a session has seats, open items or conditions, and well under
/// anything worth worrying about.
pub(super) const MAX_ID_LIST_ITEMS: usize = 100;

/// An object MADE does not look inside — a context, an output, an
/// evidence request, a payload, a bag of attributes.
///
/// Open means open, with one exception a caller has to be told about:
/// the contract carries these as `google.protobuf.Struct`, whose
/// numbers are doubles, so what counts as a number is decided at
/// ingress on every backend. The rule travels with the field rather
/// than living in a page somebody has to find (issue #75).
pub(super) fn attributes_schema(description: &str) -> Value {
    json!({
        "type": "object",
        "additionalProperties": true,
        "description": format!("{description} {STRUCT_NUMBER_RULE}")
    })
}

// ---------------------------------------------------------------------------
// Primitive helpers
// ---------------------------------------------------------------------------

#[allow(clippy::needless_pass_by_value)] // json! consumes via macro clone — clippy can't see that
pub(super) fn tool_def(name: &str, description: &str, input_schema: Value) -> Value {
    json!({
        "name": name,
        "description": description,
        "inputSchema": input_schema,
    })
}

pub(super) fn string_schema(description: &str) -> Value {
    json!({
        "type": "string",
        "minLength": 1,
        "description": description,
    })
}

#[cfg(all(test, feature = "embedded"))]
mod tests {
    use super::MAX_ID_LIST_ITEMS;
    use made_app::usecases::CeremonyReportIds;
    use made_core::value_objects::{InterventionRoleIds, ReconsiderationConditions};

    #[test]
    fn list_schemas_publish_the_limits_enforced_by_the_shared_values() {
        // grpc-only has no domain dependency. Pin its schema constant in the
        // embedded/default build instead of pulling the engine into that binary.
        assert_eq!(MAX_ID_LIST_ITEMS, CeremonyReportIds::MAX_ITEMS);
        assert_eq!(MAX_ID_LIST_ITEMS, ReconsiderationConditions::MAX_ITEMS);
        assert_eq!(MAX_ID_LIST_ITEMS, InterventionRoleIds::MAX_ITEMS);
    }
}
