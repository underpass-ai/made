use serde_json::Value;

/// Validated arguments of `made_get_status`.
///
/// One optional flag, and the reason it is a type rather than a bare
/// read is the reason every other arm has one: the request gate has
/// already checked the argument against the schema, and what is left
/// is turning the caller's JSON into what the use case takes. An
/// absent flag means the caller did not ask for the counters, which
/// is how the gRPC arm has always read it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct EmbeddedGetStatusRequest {
    include_statistics: bool,
}

impl EmbeddedGetStatusRequest {
    pub(super) const fn include_statistics(self) -> bool {
        self.include_statistics
    }
}

impl TryFrom<&Value> for EmbeddedGetStatusRequest {
    type Error = String;

    fn try_from(value: &Value) -> Result<Self, Self::Error> {
        let object = value
            .as_object()
            .ok_or_else(|| "tools/call.arguments must be an object".to_owned())?;
        let include_statistics = match object.get("include_stats") {
            None | Some(Value::Null) => false,
            Some(Value::Bool(asked)) => *asked,
            Some(_) => return Err("field `include_stats` must be a boolean".to_owned()),
        };
        Ok(Self { include_statistics })
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn a_caller_that_says_nothing_is_not_asking_for_the_counters() {
        let request = EmbeddedGetStatusRequest::try_from(&json!({})).unwrap();
        assert!(!request.include_statistics());
    }

    #[test]
    fn the_flag_is_read_as_the_caller_set_it() {
        assert!(
            EmbeddedGetStatusRequest::try_from(&json!({ "include_stats": true }))
                .unwrap()
                .include_statistics()
        );
        assert!(
            !EmbeddedGetStatusRequest::try_from(&json!({ "include_stats": false }))
                .unwrap()
                .include_statistics()
        );
    }

    #[test]
    fn a_flag_that_is_not_a_boolean_is_the_callers_to_fix() {
        let error =
            EmbeddedGetStatusRequest::try_from(&json!({ "include_stats": "yes" })).unwrap_err();
        assert!(error.contains("include_stats"), "{error}");
    }

    #[test]
    fn arguments_that_are_not_an_object_are_refused() {
        assert!(EmbeddedGetStatusRequest::try_from(&json!([])).is_err());
    }
}
