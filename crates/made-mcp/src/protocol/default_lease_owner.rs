/// The lease owner this MCP server puts on a step when the caller left
/// the field out.
///
/// `made-mcp:<backend>` — one rule, applied on every backend, before
/// the call leaves this layer. Two things follow from applying it here
/// rather than letting each side fall back to its own default.
///
/// A step lease answers "who is holding this, and may I take it".
/// Before this, an omitted owner became `made-mcp-embedded` in process,
/// `made-mcp-external-host` for a claim, and whatever the server
/// invented over the wire — three answers to one question, none of
/// which a client could predict. The server keeps its own default for
/// clients that speak gRPC directly; from here it is never reached,
/// because this layer always sends a value.
///
/// The backend is in the id because it is the operationally useful
/// half: `made-mcp:grpc` and `made-mcp:embedded` tell an operator
/// reading a held lease which kind of process is holding it. What is
/// the same on every backend is the rule, and a client reads the owner
/// the same way whichever one answered.
#[cfg(any(feature = "embedded", feature = "grpc", test))]
pub(crate) fn default_lease_owner_id(backend_name: &str) -> String {
    format!("made-mcp:{backend_name}")
}

/// The rule in the words the tool schemas use, so the three
/// descriptions cannot drift from each other or from the code.
pub(crate) const DEFAULT_LEASE_OWNER_RULE: &str =
    "Optional logical runner acquiring the step lease. Omitted, this MCP \
     server applies `made-mcp:<backend>` — `made-mcp:embedded` in process, \
     `made-mcp:grpc` against a cluster — before the call reaches the \
     engine, so the same omission means the same owner on every backend.";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_rule_names_the_backend_that_applied_it() {
        assert_eq!(default_lease_owner_id("embedded"), "made-mcp:embedded");
        assert_eq!(default_lease_owner_id("grpc"), "made-mcp:grpc");
    }

    #[test]
    fn the_schemas_state_the_rule_they_implement() {
        assert!(DEFAULT_LEASE_OWNER_RULE.contains("made-mcp:<backend>"));
        assert!(DEFAULT_LEASE_OWNER_RULE.contains(&default_lease_owner_id("embedded")));
        assert!(DEFAULT_LEASE_OWNER_RULE.contains(&default_lease_owner_id("grpc")));
    }
}
