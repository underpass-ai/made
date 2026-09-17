use made_app::usecases::ReadCeremonyEventsInput;
use made_core::value_objects::{CeremonyEventPageLimit, CeremonyId, StreamVersion};
use serde_json::Value;

use super::embedded_request_fields::{optional_u64, required_string};

/// Validated MCP request for one page of a ceremony's event stream.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct EmbeddedReadCeremonyEventsRequest {
    ceremony_id: CeremonyId,
    from_version: StreamVersion,
    limit: Option<usize>,
}

impl EmbeddedReadCeremonyEventsRequest {
    /// Fallible for one reason: a limit above the cap is refused by the
    /// engine, as the tool's own schema says (`maximum: 1000`) and as
    /// the request gate already refuses before this is ever built. The
    /// two agree by saying the same thing rather than by one of them
    /// quietly making the other's answer smaller.
    pub(super) fn into_input(self) -> Result<ReadCeremonyEventsInput, String> {
        let limit = self
            .limit
            .map(CeremonyEventPageLimit::new)
            .transpose()
            .map_err(|error| error.to_string())?
            .unwrap_or_default();
        Ok(ReadCeremonyEventsInput::new(
            self.ceremony_id,
            self.from_version,
            limit,
        ))
    }
}

impl TryFrom<&Value> for EmbeddedReadCeremonyEventsRequest {
    type Error = String;

    fn try_from(value: &Value) -> Result<Self, Self::Error> {
        let object = value
            .as_object()
            .ok_or_else(|| "tools/call.arguments must be an object".to_owned())?;
        let from_version = optional_u64(object, "from_version")?.unwrap_or(0);
        // Zero is "not asked for" on both fields: nobody means "read no
        // records", and the version already seen starts at zero.
        let limit = optional_u64(object, "limit")?
            .filter(|limit| *limit > 0)
            .map(|limit| usize::try_from(limit).unwrap_or(usize::MAX));
        Ok(Self {
            ceremony_id: CeremonyId::new(required_string(object, "ceremony_id")?)
                .map_err(|error| error.to_string())?,
            from_version: StreamVersion::new(from_version),
            limit,
        })
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn a_request_with_only_an_id_reads_from_the_beginning_with_the_default_limit() {
        let request =
            EmbeddedReadCeremonyEventsRequest::try_from(&json!({ "ceremony_id": "session-1" }))
                .unwrap()
                .into_input()
                .unwrap();

        assert_eq!(request.from_version(), StreamVersion::EMPTY);
        assert_eq!(
            request.limit().value(),
            ReadCeremonyEventsInput::DEFAULT_LIMIT
        );
    }

    #[test]
    fn a_version_and_a_limit_are_carried_through() {
        let request = EmbeddedReadCeremonyEventsRequest::try_from(&json!({
            "ceremony_id": "session-1",
            "from_version": 7,
            "limit": 3,
        }))
        .unwrap()
        .into_input()
        .unwrap();

        assert_eq!(request.from_version(), StreamVersion::new(7));
        assert_eq!(request.limit().value(), 3);
    }

    #[test]
    fn a_zero_limit_is_the_default_rather_than_an_empty_page() {
        let request = EmbeddedReadCeremonyEventsRequest::try_from(
            &json!({ "ceremony_id": "session-1", "limit": 0 }),
        )
        .unwrap()
        .into_input()
        .unwrap();

        assert_eq!(
            request.limit().value(),
            ReadCeremonyEventsInput::DEFAULT_LIMIT
        );
    }

    /// The engine refuses what the schema refuses, so a caller cannot
    /// reach one answer through the gate and a different one past it.
    #[test]
    fn a_limit_above_the_cap_is_refused_rather_than_clamped() {
        let error = EmbeddedReadCeremonyEventsRequest::try_from(&json!({
            "ceremony_id": "session-1",
            "limit": 1_001,
        }))
        .unwrap()
        .into_input()
        .expect_err("a page bigger than a page is not a page");

        assert!(error.contains("limit"), "{error}");
    }

    #[test]
    fn an_empty_ceremony_id_is_refused() {
        assert!(
            EmbeddedReadCeremonyEventsRequest::try_from(&json!({ "ceremony_id": "" })).is_err()
        );
    }
}
