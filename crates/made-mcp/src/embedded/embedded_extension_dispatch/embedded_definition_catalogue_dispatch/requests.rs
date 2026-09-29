//! Reading the two catalogue requests off the wire, into the values
//! the facade takes.

use made_app::usecases::PublishedCeremonyDefinitionQuery;
use made_core::value_objects::{
    CeremonyDefinitionCursor, CeremonyDefinitionPageLimit, CeremonyName, CeremonyVersion,
};
use serde_json::Value;

use crate::protocol::ToolError;

pub(super) fn query(arguments: &Value) -> Result<PublishedCeremonyDefinitionQuery, ToolError> {
    let ceremony = optional_text(arguments, "ceremony")
        .map(CeremonyName::new)
        .transpose()?;
    let limit = match arguments.get("limit") {
        None | Some(Value::Null) => CeremonyDefinitionPageLimit::default(),
        Some(value) => {
            let raw = value
                .as_u64()
                .and_then(|raw| u16::try_from(raw).ok())
                .ok_or_else(|| ToolError::invalid_request("field `limit` is out of range"))?;
            CeremonyDefinitionPageLimit::new(raw)?
        }
    };
    let after = optional_text(arguments, "cursor")
        .map(CeremonyDefinitionCursor::parse)
        .transpose()?;
    Ok(PublishedCeremonyDefinitionQuery::new(
        ceremony, limit, after,
    ))
}

pub(super) fn identity(arguments: &Value) -> Result<(CeremonyName, CeremonyVersion), ToolError> {
    Ok((
        CeremonyName::new(required_text(arguments, "ceremony")?)?,
        CeremonyVersion::new(required_text(arguments, "version")?)?,
    ))
}

/// Empty is absent, as it is on the wire the other backend speaks.
fn optional_text<'a>(arguments: &'a Value, field: &str) -> Option<&'a str> {
    arguments
        .get(field)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

fn required_text<'a>(arguments: &'a Value, field: &str) -> Result<&'a str, ToolError> {
    arguments
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| ToolError::invalid_request(format!("field `{field}` is required")))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn nothing_asks_for_the_first_default_page() {
        assert_eq!(
            query(&json!({})).unwrap(),
            PublishedCeremonyDefinitionQuery::default()
        );
        assert_eq!(
            query(&json!({ "ceremony": "", "cursor": "" })).unwrap(),
            PublishedCeremonyDefinitionQuery::default()
        );
    }

    #[test]
    fn a_name_a_limit_and_a_cursor_are_read_into_their_values() {
        let read =
            query(&json!({ "ceremony": "review", "limit": 3, "cursor": "review@1.0" })).unwrap();

        assert_eq!(read.ceremony().unwrap().as_str(), "review");
        assert_eq!(read.limit().get(), 3);
        assert_eq!(read.after().unwrap().to_string(), "review@1.0");
    }

    #[test]
    fn an_unusable_limit_or_cursor_is_refused() {
        assert!(query(&json!({ "limit": 0 })).is_err());
        assert!(query(&json!({ "limit": 70_000 })).is_err());
        assert!(query(&json!({ "cursor": "review" })).is_err());
    }

    #[test]
    fn a_read_needs_both_halves_of_the_identity() {
        assert!(identity(&json!({ "ceremony": "review", "version": "1.0" })).is_ok());
        assert!(identity(&json!({ "ceremony": "review" })).is_err());
        assert!(identity(&json!({ "version": "1.0" })).is_err());
    }
}
