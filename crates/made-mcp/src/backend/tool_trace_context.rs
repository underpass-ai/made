use uuid::Uuid;

/// Validated W3C traceparent metadata carried by one MCP tool call.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ToolTraceContext(String);

impl ToolTraceContext {
    #[must_use]
    pub fn from_metadata(traceparent: Option<&str>) -> Self {
        traceparent
            .filter(|value| valid_traceparent(value))
            .map_or_else(Self::generate, |value| Self(value.to_ascii_lowercase()))
    }

    #[must_use]
    pub fn traceparent(&self) -> &str {
        &self.0
    }

    fn generate() -> Self {
        let trace_id = Uuid::new_v4().simple();
        let span = Uuid::new_v4().as_u128();
        Self(format!("00-{trace_id}-{:016x}-01", span >> 64))
    }
}

fn valid_traceparent(value: &str) -> bool {
    let parts = value.split('-').collect::<Vec<_>>();
    parts.len() == 4
        && parts[0].len() == 2
        && parts[1].len() == 32
        && parts[2].len() == 16
        && parts[3].len() == 2
        && parts
            .iter()
            .all(|part| part.bytes().all(|byte| byte.is_ascii_hexdigit()))
        && parts[1].bytes().any(|byte| byte != b'0')
        && parts[2].bytes().any(|byte| byte != b'0')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_valid_metadata_and_replaces_invalid_or_absent_values() {
        let supplied = "00-0af7651916cd43dd8448eb211c80319c-b7ad6b7169203331-01";
        assert_eq!(
            ToolTraceContext::from_metadata(Some(supplied)).traceparent(),
            supplied
        );
        for minted in [
            ToolTraceContext::from_metadata(None),
            ToolTraceContext::from_metadata(Some("bad")),
        ] {
            assert!(valid_traceparent(minted.traceparent()));
        }
    }
}
