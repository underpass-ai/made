//! One request gate, before dispatch, for every backend.
//!
//! A tool publishes an input schema in `tools/list`. Whether a call is
//! accepted must follow from that schema and from nothing else — not
//! from which backend happens to be mounted, and not from how far a
//! request mapper happens to look. Before this gate the two arms
//! disagreed in both directions: the gRPC arm dropped a field it did
//! not recognise on the floor and let a non-string past where the
//! in-process arm refused it, and each arm worded the refusal its own
//! way.
//!
//! So the check runs once, in the server layer, on the arguments as
//! they arrive: the caller gets the same answer from the same call
//! whichever engine is behind it, byte for byte, because the engine is
//! never reached. Everything here is the call's own fault, so
//! everything here is `invalid_request` (ADR-014, plan §3.6 F4).
//!
//! The subset of JSON Schema the catalog actually writes — `type`,
//! `required`, `properties`, `additionalProperties`, `minProperties`,
//! `enum`, `minLength`, `minItems`, `maxItems`, `uniqueItems`, `items`,
//! `minimum`, `maximum`, `oneOf`, `anyOf`, `not`, `if`/`then`/`else` —
//! is validated; a keyword no schema uses is ignored, as JSON Schema
//! says to. A tool whose schema grows a keyword this does not know is
//! covered by the schema itself, not by a list kept here.
//!
//! The gate also hands back the arguments it accepted, because two of
//! the things a published schema means are decisions about the call
//! rather than complaints about it, and both have to be made in one
//! place or the two arms make them differently:
//!
//! - **An explicit `null` on a field the schema does not require is
//!   absent.** A host generated from a typed SDK serialises an unset
//!   optional as `null`, and refusing that made a correct client look
//!   broken. It is dropped here, so neither request mapper has to have
//!   an opinion about it.
//! - **A top-level key starting with `_` is not an argument.** MCP
//!   reserves `_meta` on any object it defines, hosts attach it, and
//!   `additionalProperties: false` refused every call that carried one.
//!   Skipped at `tools/call.arguments`, and only there: inside an open
//!   payload the caller's keys are the caller's.

use std::collections::BTreeSet;

use serde_json::{Map, Value};

use super::catalog::available_tool_catalog;
use super::tool_error::ToolError;

/// Where a complaint about the arguments starts, so a message reads
/// the same way as the request the client sent.
const ARGUMENTS: &str = "tools/call.arguments";

/// Refuse a `tools/call` the published catalog does not admit, and
/// answer with the arguments a backend should be handed.
///
/// `supports` is the active backend's own answer, so the catalog this
/// validates against is exactly the one `tools/list` served.
pub(crate) fn validate_tool_request(
    name: &str,
    arguments: &Value,
    supports: impl Fn(&str) -> bool,
) -> Result<Value, ToolError> {
    let catalog = available_tool_catalog(supports);
    let Some(tool) = catalog
        .iter()
        .find(|tool| tool.get("name").and_then(Value::as_str) == Some(name))
    else {
        return Err(ToolError::invalid_request(format!(
            "no tool named `{name}` is served by this backend; \
             call `tools/list` for the catalog it does serve"
        )));
    };

    // An absent `arguments` is an empty argument object, not a
    // different kind of request: a tool with no required field is
    // callable either way, and one with a required field says which
    // field is missing rather than complaining about the envelope.
    let arguments = if arguments.is_null() {
        &Value::Object(serde_json::Map::new())
    } else {
        arguments
    };

    let schema = &tool["inputSchema"];
    let arguments = canonical(arguments, schema, true);
    validate(&arguments, schema, ARGUMENTS)
        .map_err(|complaint| ToolError::invalid_request(format!("{name}: {complaint}")))?;
    Ok(arguments)
}

/// The arguments as this layer reads them: an unset optional dropped
/// rather than carried as a `null`, and the host's own `_`-prefixed
/// keys left behind at the top.
///
/// A field the schema does not declare is kept exactly as it arrived,
/// so `additionalProperties: false` still names it below.
fn canonical(value: &Value, schema: &Value, at_the_top: bool) -> Value {
    match value {
        Value::Object(fields) => {
            let required: BTreeSet<&str> = schema
                .get("required")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .collect();
            let properties = schema.get("properties").and_then(Value::as_object);
            let mut kept = Map::new();
            for (field, child) in fields {
                if at_the_top && field.starts_with('_') {
                    continue;
                }
                match properties.and_then(|properties| properties.get(field)) {
                    Some(property) => {
                        if child.is_null() && !required.contains(field.as_str()) {
                            continue;
                        }
                        kept.insert(field.clone(), canonical(child, property, false));
                    }
                    None => {
                        kept.insert(field.clone(), child.clone());
                    }
                }
            }
            Value::Object(kept)
        }
        Value::Array(items) => match schema.get("items") {
            Some(item_schema) => Value::Array(
                items
                    .iter()
                    .map(|item| canonical(item, item_schema, false))
                    .collect(),
            ),
            None => value.clone(),
        },
        leaf => leaf.clone(),
    }
}

/// One value against one schema. `path` is what the caller called it.
fn validate(value: &Value, schema: &Value, path: &str) -> Result<(), String> {
    check_type(value, schema, path)?;
    check_enum(value, schema, path)?;
    check_string(value, schema, path)?;
    check_number(value, schema, path)?;
    check_array(value, schema, path)?;
    check_object(value, schema, path)?;
    check_combinators(value, schema, path)
}

fn check_type(value: &Value, schema: &Value, path: &str) -> Result<(), String> {
    let Some(declared) = schema.get("type").and_then(Value::as_str) else {
        return Ok(());
    };
    let matches = match declared {
        "object" => value.is_object(),
        "array" => value.is_array(),
        "string" => value.is_string(),
        "boolean" => value.is_boolean(),
        "integer" => value.is_i64() || value.is_u64(),
        "number" => value.is_number(),
        "null" => value.is_null(),
        _ => true,
    };
    if matches {
        Ok(())
    } else {
        Err(format!(
            "`{path}` must be {}, not {}",
            article(declared),
            article(kind_of(value))
        ))
    }
}

fn check_enum(value: &Value, schema: &Value, path: &str) -> Result<(), String> {
    let Some(variants) = schema.get("enum").and_then(Value::as_array) else {
        return Ok(());
    };
    if variants.contains(value) {
        return Ok(());
    }
    let allowed = variants
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(", ");
    Err(format!(
        "`{path}` is {value}, which is not one of {allowed}"
    ))
}

fn check_string(value: &Value, schema: &Value, path: &str) -> Result<(), String> {
    let (Some(text), Some(minimum)) = (
        value.as_str(),
        schema.get("minLength").and_then(Value::as_u64),
    ) else {
        return Ok(());
    };
    if u64::try_from(text.chars().count()).unwrap_or(u64::MAX) >= minimum {
        Ok(())
    } else if minimum == 1 {
        Err(format!("`{path}` must not be empty"))
    } else {
        Err(format!("`{path}` must be at least {minimum} characters"))
    }
}

fn check_number(value: &Value, schema: &Value, path: &str) -> Result<(), String> {
    let Some(number) = value.as_f64() else {
        return Ok(());
    };
    if let Some(minimum) = schema.get("minimum").and_then(Value::as_f64) {
        if number < minimum {
            return Err(format!("`{path}` is {number}, below the minimum {minimum}"));
        }
    }
    if let Some(maximum) = schema.get("maximum").and_then(Value::as_f64) {
        if number > maximum {
            return Err(format!("`{path}` is {number}, above the maximum {maximum}"));
        }
    }
    Ok(())
}

fn check_array(value: &Value, schema: &Value, path: &str) -> Result<(), String> {
    let Some(items) = value.as_array() else {
        return Ok(());
    };
    if let Some(minimum) = schema.get("minItems").and_then(Value::as_u64) {
        if u64::try_from(items.len()).unwrap_or(u64::MAX) < minimum {
            return Err(format!(
                "`{path}` has {} entries, fewer than the {minimum} this tool requires",
                items.len()
            ));
        }
    }
    if let Some(maximum) = schema.get("maxItems").and_then(Value::as_u64) {
        if u64::try_from(items.len()).unwrap_or(u64::MAX) > maximum {
            return Err(format!(
                "`{path}` has {} entries, more than the {maximum} this tool accepts",
                items.len()
            ));
        }
    }
    if schema.get("uniqueItems") == Some(&Value::Bool(true)) {
        // A set, not a scan of what came before: every array that
        // declares uniqueness also declares `maxItems`, so the cost is
        // bounded either way, but n² on a caller-supplied list is work
        // a caller decides the size of.
        let mut seen: BTreeSet<String> = BTreeSet::new();
        for item in items {
            if !seen.insert(item.to_string()) {
                return Err(format!(
                    "`{path}` lists {item} more than once, and duplicates are refused"
                ));
            }
        }
    }
    if let Some(item_schema) = schema.get("items") {
        for (index, item) in items.iter().enumerate() {
            validate(item, item_schema, &format!("{path}[{index}]"))?;
        }
    }
    Ok(())
}

fn check_object(value: &Value, schema: &Value, path: &str) -> Result<(), String> {
    let Some(fields) = value.as_object() else {
        return Ok(());
    };
    if let Some(minimum) = schema.get("minProperties").and_then(Value::as_u64) {
        if u64::try_from(fields.len()).unwrap_or(u64::MAX) < minimum {
            return Err(if minimum == 1 {
                format!("`{path}` must not be empty")
            } else {
                format!(
                    "`{path}` has {} entries, fewer than the {minimum} this tool requires",
                    fields.len()
                )
            });
        }
    }
    for field in schema
        .get("required")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
    {
        if !fields.contains_key(field) {
            return Err(format!("`{path}` is missing the required field `{field}`"));
        }
    }
    let properties = schema.get("properties").and_then(Value::as_object);
    for (field, child) in fields {
        let child_path = format!("{path}.{field}");
        match properties.and_then(|properties| properties.get(field)) {
            Some(property) => validate(child, property, &child_path)?,
            None => match schema.get("additionalProperties") {
                Some(Value::Bool(false)) => {
                    return Err(format!(
                        "`{path}` carries `{field}`, which this tool does not declare"
                    ))
                }
                Some(additional) if additional.is_object() => {
                    validate(child, additional, &child_path)?;
                }
                _ => {}
            },
        }
    }
    Ok(())
}

/// `oneOf`, `anyOf` and `not`. The catalog states alternative
/// requirements this way — one of `council_id` / `specialty` and not
/// the other — and a schema that states its alternatives is a schema
/// a caller can satisfy without reading prose.
fn check_combinators(value: &Value, schema: &Value, path: &str) -> Result<(), String> {
    if let Some(branches) = schema.get("anyOf").and_then(Value::as_array) {
        if !branches
            .iter()
            .any(|branch| validate(value, branch, path).is_ok())
        {
            return Err(format!(
                "`{path}` satisfies none of the {} alternatives the tool declares",
                branches.len()
            ));
        }
    }
    if let Some(branches) = schema.get("oneOf").and_then(Value::as_array) {
        let satisfied = branches
            .iter()
            .filter(|branch| validate(value, branch, path).is_ok())
            .count();
        if satisfied != 1 {
            return Err(format!(
                "`{path}` satisfies {satisfied} of the {} mutually exclusive \
                 alternatives the tool declares, and exactly one is required",
                branches.len()
            ));
        }
    }
    if let Some(forbidden) = schema.get("not") {
        if validate(value, forbidden, path).is_ok() {
            return Err(format!(
                "`{path}` is what the tool declares this alternative excludes"
            ));
        }
    }
    // `if`/`then`/`else`, which is how a schema says "this field
    // depends on that one". The branch's own complaint is returned
    // rather than a sentence about the combinator, because "missing the
    // required field `error`" is what the caller has to act on.
    if let Some(condition) = schema.get("if") {
        let branch = if validate(value, condition, path).is_ok() {
            schema.get("then")
        } else {
            schema.get("else")
        };
        if let Some(branch) = branch {
            validate(value, branch, path)?;
        }
    }
    Ok(())
}

fn kind_of(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

fn article(kind: &str) -> String {
    if kind.starts_with(['a', 'e', 'i', 'o', 'u']) {
        format!("an {kind}")
    } else {
        format!("a {kind}")
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::protocol::schema_primitives::MAX_ID_LIST_ITEMS;
    use crate::protocol::ToolErrorCode;

    /// The catalog every backend admits, so these exercise the gate
    /// rather than one backend's share of it.
    fn gate(name: &str, arguments: &Value) -> Result<Value, ToolError> {
        validate_tool_request(name, arguments, |_| true)
    }

    fn complaint(name: &str, arguments: &Value) -> String {
        let error = gate(name, arguments).expect_err("these arguments should be refused");
        assert_eq!(error.code(), ToolErrorCode::InvalidRequest);
        error.message().to_owned()
    }

    #[test]
    fn a_tool_the_catalog_does_not_serve_is_refused() {
        let message = complaint("made_do_something_else", &json!({}));
        assert!(
            message.contains("no tool named `made_do_something_else`"),
            "{message}"
        );
    }

    #[test]
    fn a_tool_this_backend_does_not_serve_is_refused_the_same_way() {
        let error = validate_tool_request("made_get_status", &json!({}), |_| false)
            .expect_err("a backend that serves nothing serves this either");
        assert_eq!(error.code(), ToolErrorCode::InvalidRequest);
    }

    #[test]
    fn a_missing_required_field_names_itself() {
        let message = complaint("made_get_ceremony_instance", &json!({}));
        assert!(
            message.contains("missing the required field `ceremony_id`"),
            "{message}"
        );
    }

    #[test]
    fn absent_arguments_read_as_an_empty_object() {
        let message = complaint("made_get_ceremony_instance", &Value::Null);
        assert!(
            message.contains("missing the required field `ceremony_id`"),
            "{message}"
        );
        gate("made_list_ceremony_instances", &Value::Null)
            .expect("a tool with no required field is callable with no arguments");
    }

    #[test]
    fn a_field_of_the_wrong_type_is_refused() {
        let message = complaint("made_get_ceremony_instance", &json!({ "ceremony_id": 7 }));
        assert!(
            message.contains("must be a string, not a number"),
            "{message}"
        );
    }

    #[test]
    fn an_empty_string_where_the_schema_demands_one_is_refused() {
        let message = complaint("made_get_ceremony_instance", &json!({ "ceremony_id": "" }));
        assert!(message.contains("must not be empty"), "{message}");
    }

    #[test]
    fn a_value_outside_a_declared_enum_is_refused() {
        let message = complaint(
            "made_apply_ceremony_transition",
            &json!({ "ceremony_id": "s", "trigger": "opened", "actor_kind": "wizard" }),
        );
        assert!(message.contains("not one of"), "{message}");
    }

    #[test]
    fn a_field_the_tool_does_not_declare_is_refused() {
        let message = complaint(
            "made_get_ceremony_instance",
            &json!({ "ceremony_id": "s", "ceremony_di": "s" }),
        );
        assert!(
            message.contains("carries `ceremony_di`, which this tool does not declare"),
            "{message}"
        );
    }

    #[test]
    fn an_empty_entry_inside_a_declared_array_is_refused() {
        let message = complaint(
            "made_defer_ceremony_guard",
            &json!({
                "ceremony_id": "s",
                "guard_name": "g",
                "role_id": "R",
                "role_kind": "human",
                "statement": "not yet",
                "reason": "away",
                "reconsider_when": [""],
            }),
        );
        assert!(message.contains("reconsider_when[0]"), "{message}");
    }

    #[test]
    fn an_integer_below_its_minimum_is_refused() {
        let message = complaint(
            "made_run_ceremony_step",
            &json!({
                "ceremony_id": "s",
                "step_id": "work",
                "actor_kind": "agent",
                "lease_ttl_ms": -1,
            }),
        );
        assert!(message.contains("below the minimum"), "{message}");
    }

    #[test]
    fn mutually_exclusive_alternatives_demand_exactly_one() {
        let both = complaint(
            "made_run_council_decision",
            &json!({
                "contract_id": "c",
                "description": "d",
                "council_id": "one",
                "specialty": "other",
            }),
        );
        assert!(both.contains("mutually exclusive"), "{both}");
        let neither = complaint(
            "made_run_council_decision",
            &json!({ "contract_id": "c", "description": "d" }),
        );
        assert!(neither.contains("mutually exclusive"), "{neither}");
        gate(
            "made_run_council_decision",
            &json!({ "contract_id": "c", "description": "d", "specialty": "review" }),
        )
        .expect("one alternative and not the other is what the schema asks for");
    }

    #[test]
    fn a_free_form_object_takes_whatever_the_caller_puts_in_it() {
        gate(
            "made_start_ceremony",
            &json!({
                "definition_yaml": "version: \"1.0\"",
                "actor_id": "operator",
                "actor_kind": "service",
                "context": { "anything": { "nested": [1, 2, 3] } },
            }),
        )
        .expect("`context` is declared open, and open means open");
    }

    /// A host generated from a typed SDK serialises an unset optional
    /// as `null`. Refusing that made a correct client look broken, and
    /// it was refused on both arms, so pointing it somewhere else did
    /// not help.
    #[test]
    fn an_explicit_null_on_an_optional_field_is_absent() {
        let accepted = gate(
            "made_run_ceremony_step",
            &json!({
                "ceremony_id": "s",
                "step_id": "work",
                "actor_kind": "agent",
                "lease_owner_id": null,
                "idempotency_key": null,
                "lease_ttl_ms": null,
            }),
        )
        .expect("an unset optional is unset, however a client spells it");

        assert_eq!(
            accepted,
            json!({ "ceremony_id": "s", "step_id": "work", "actor_kind": "agent" }),
            "the null must not reach a request mapper, which would have to have \
             its own opinion about it"
        );
    }

    /// A required field is not optional because it was written as
    /// `null`: it is a value of the wrong type, and it is named.
    #[test]
    fn an_explicit_null_on_a_required_field_is_still_refused() {
        let message = complaint(
            "made_get_ceremony_instance",
            &json!({ "ceremony_id": null }),
        );
        assert!(
            message.contains("must be a string, not a null"),
            "{message}"
        );
    }

    /// MCP reserves `_meta` on any object it defines, and hosts attach
    /// it. `additionalProperties: false` refused every call that
    /// carried one.
    #[test]
    fn a_host_s_own_underscored_key_is_skipped_at_the_top() {
        let accepted = gate(
            "made_get_ceremony_instance",
            &json!({
                "ceremony_id": "s",
                "_meta": { "progressToken": 7 },
            }),
        )
        .expect("`_meta` is the host's, not an argument");
        assert_eq!(accepted, json!({ "ceremony_id": "s" }));
    }

    /// Only at the top. Inside an open payload the keys are the
    /// caller's, underscore or not, and dropping one would lose data
    /// the engine is meant to seal.
    #[test]
    fn an_underscored_key_inside_an_open_payload_is_the_callers_own() {
        let accepted = gate(
            "made_start_ceremony",
            &json!({
                "definition_yaml": "version: \"1.0\"",
                "actor_id": "operator",
                "actor_kind": "service",
                "context": { "_internal_ref": "INC-42" },
            }),
        )
        .expect("open means open");
        assert_eq!(accepted["context"], json!({ "_internal_ref": "INC-42" }));
    }

    #[test]
    fn a_list_of_ids_longer_than_the_bound_is_refused() {
        let many: Vec<String> = (0..=MAX_ID_LIST_ITEMS)
            .map(|index| format!("session-{index}"))
            .collect();
        let message = complaint(
            "made_generate_ceremony_report",
            &json!({ "ceremony_ids": many }),
        );
        assert!(message.contains("more than the"), "{message}");
    }

    /// The three constraints the descriptions used to promise in prose
    /// only, now stated where a caller's own validator can read them.
    #[test]
    fn exactly_one_way_of_naming_a_definition_is_accepted() {
        let both = complaint(
            "made_diff_ceremony_definitions",
            &json!({
                "before": { "ceremony": "c", "version": "1.0", "definition_yaml": "x" },
                "after": { "definition_yaml": "y" },
            }),
        );
        assert!(both.contains("before"), "{both}");

        let neither = complaint(
            "made_diff_ceremony_definitions",
            &json!({ "before": {}, "after": { "definition_yaml": "y" } }),
        );
        assert!(neither.contains("before"), "{neither}");

        gate(
            "made_diff_ceremony_definitions",
            &json!({
                "before": { "ceremony": "c", "version": "1.0" },
                "after": { "definition_yaml": "y" },
            }),
        )
        .expect("a published version on one side and a document on the other is the point");
    }

    #[test]
    fn a_table_with_nobody_at_it_is_refused() {
        let message = complaint(
            "made_bind_ceremony_participants",
            &json!({
                "ceremony_id": "s",
                "seating": {},
                "actor_id": "operator",
                "actor_kind": "service",
            }),
        );
        assert!(message.contains("seating"), "{message}");
        assert!(message.contains("must not be empty"), "{message}");
    }

    #[test]
    fn a_failed_step_must_say_why_and_a_successful_one_must_not() {
        let silent = complaint(
            "made_complete_ceremony_step",
            &json!({
                "ceremony_id": "s",
                "step_id": "work",
                "actor_kind": "agent",
                "status": "failed",
            }),
        );
        assert!(silent.contains("required field `error`"), "{silent}");

        let contradictory = complaint(
            "made_complete_ceremony_step",
            &json!({
                "ceremony_id": "s",
                "step_id": "work",
                "actor_kind": "agent",
                "status": "completed",
                "error": "but it worked",
            }),
        );
        assert!(contradictory.contains("excludes"), "{contradictory}");

        for accepted in [
            json!({
                "ceremony_id": "s",
                "step_id": "work",
                "actor_kind": "agent",
                "status": "failed",
                "error": "the queue never drained",
            }),
            json!({
                "ceremony_id": "s",
                "step_id": "work",
                "actor_kind": "agent",
                "status": "completed",
            }),
        ] {
            gate("made_complete_ceremony_step", &accepted)
                .unwrap_or_else(|error| panic!("{accepted} should be accepted: {error}"));
        }
    }

    #[test]
    fn omitting_an_optional_field_is_accepted() {
        gate(
            "made_run_ceremony_step",
            &json!({ "ceremony_id": "s", "step_id": "work", "actor_kind": "agent" }),
        )
        .expect("the lease owner, the idempotency key and the TTL are all optional");
    }
}
