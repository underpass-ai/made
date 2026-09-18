use std::collections::BTreeSet;

use serde_json::{Map, Value};

/// Validate P5 stage fields before either MCP backend maps its request.
pub(crate) fn validate_design_dynamic_fields(arguments: &Value) -> Result<(), String> {
    let Some(stages) = arguments
        .as_object()
        .and_then(|object| object.get("stages"))
        .and_then(Value::as_array)
    else {
        return Ok(());
    };
    for stage in stages {
        let Some(stage) = stage.as_object() else {
            continue;
        };
        if let Some(group) = stage.get("group").and_then(Value::as_object) {
            if ["role_from", "allowed_roles", "context_writes"]
                .iter()
                .any(|field| stage.contains_key(*field))
            {
                return Err(
                    "a group container cannot declare role_from, allowed_roles or context_writes"
                        .to_owned(),
                );
            }
            let Some(steps) = group.get("steps").and_then(Value::as_array) else {
                continue;
            };
            for step in steps.iter().filter_map(Value::as_object) {
                validate_stage(step)?;
            }
        } else {
            validate_stage(stage)?;
        }
    }
    Ok(())
}

fn validate_stage(stage: &Map<String, Value>) -> Result<(), String> {
    let role_from = match stage.get("role_from") {
        None => None,
        Some(Value::String(value))
            if value.strip_prefix("context.").is_some_and(|key| {
                !key.trim().is_empty() && !key.chars().any(char::is_control)
            }) =>
        {
            Some(value)
        }
        Some(Value::String(value)) if value.trim().is_empty() => {
            return Err("field `role_from` must not be blank".to_owned());
        }
        Some(Value::String(_)) => {
            return Err("field `role_from` must use context.<key>".to_owned());
        }
        Some(_) => return Err("field `role_from` must be a string".to_owned()),
    };
    let allowed_roles = match stage.get("allowed_roles") {
        None => None,
        Some(Value::Array(values)) if !values.is_empty() => {
            let mut seen = BTreeSet::new();
            for value in values {
                let Some(role) = value.as_str() else {
                    return Err("field `allowed_roles` must contain only strings".to_owned());
                };
                if role.trim().is_empty() {
                    return Err("field `allowed_roles` must not contain blank strings".to_owned());
                }
                if !seen.insert(role) {
                    return Err("field `allowed_roles` must not contain duplicates".to_owned());
                }
            }
            Some(values)
        }
        Some(Value::Array(_)) => return Err("field `allowed_roles` must not be empty".to_owned()),
        Some(_) => return Err("field `allowed_roles` must be an array of strings".to_owned()),
    };
    if role_from.is_some() != allowed_roles.is_some() {
        return Err("fields `role_from` and `allowed_roles` must be declared together".to_owned());
    }
    if let Some(value) = stage.get("context_writes") {
        let Some(entries) = value.as_object() else {
            return Err("field `context_writes` must be an object of strings".to_owned());
        };
        for (key, value) in entries {
            if key.trim().is_empty() || key.chars().any(char::is_control) {
                return Err("field `context_writes` must not contain blank keys".to_owned());
            }
            let Some(value) = value.as_str() else {
                return Err("field `context_writes` must contain only strings".to_owned());
            };
            if value.trim().is_empty() || value.chars().any(char::is_control) {
                return Err("field `context_writes` must not contain blank strings".to_owned());
            }
        }
    }
    Ok(())
}
