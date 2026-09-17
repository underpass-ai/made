//! How long a step lease lasts when the caller asked for no particular
//! length — the same three numbers on every backend.
//!
//! An omitted `lease_ttl_ms` used to reach whichever default the engine
//! on the other side happened to keep: thirty seconds for a one-shot run
//! in process, sixty over the wire, because the gRPC arm sent `0` and let
//! the server choose. Same call, same omission, two leases.
//!
//! So this layer sends the number rather than the zero, on both arms,
//! and the number is the engine's own: `made-app` declares it next to the
//! input that carries it, and the three constants below are that
//! declaration repeated where the schemas can read it. They are repeated
//! rather than imported because `made-mcp` builds without `made-app` when
//! only the gRPC backend is compiled in — the same reason
//! `DEFAULT_EVENT_PAGE_LIMIT` is written twice — and the test at the
//! bottom is what keeps the copy honest.
//!
//! The server keeps its own defaults for clients that speak gRPC
//! directly; from MCP they are never reached, because this layer always
//! sends a value.

/// One engine-run ceremony, start to finish, in one call.
pub(crate) const RUN_CEREMONY_LEASE_TTL_MS: u64 = 60_000;

/// One step the engine runs through its own handler.
pub(crate) const RUN_CEREMONY_STEP_LEASE_TTL_MS: u64 = 30_000;

/// One step the host claims and executes where the engine cannot see it.
pub(crate) const CLAIM_CEREMONY_STEP_LEASE_TTL_MS: u64 = 300_000;

/// The rule in the words a tool schema uses, with the number in it.
///
/// Built from the constant rather than written beside it, so a schema
/// cannot promise a length this server does not send.
pub(crate) fn lease_ttl_rule(default_ms: u64) -> String {
    format!(
        "Step lease TTL in milliseconds. Omitted or 0, this MCP server sends \
         {default_ms} before the call reaches the engine, so the same omission \
         means the same lease on every backend."
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_rule_states_the_number_it_applies() {
        let rule = lease_ttl_rule(RUN_CEREMONY_LEASE_TTL_MS);
        assert!(rule.contains("60000"), "{rule}");
        assert!(rule.contains("every backend"), "{rule}");
    }

    /// The three numbers are the engine's, written twice.
    ///
    /// `made-mcp` builds without `made-app` when only the gRPC backend is
    /// compiled in, so the schemas cannot read them from where they are
    /// decided. A schema that promised a lease the engine does not apply
    /// fails here rather than misleading the caller who read it.
    #[cfg(feature = "embedded")]
    #[test]
    fn the_published_lease_defaults_are_the_ones_the_engine_applies() {
        use made_app::usecases::{RunCeremonyInput, RunCeremonyStepInput, StartCeremonyStepInput};

        assert_eq!(
            RUN_CEREMONY_LEASE_TTL_MS,
            RunCeremonyInput::DEFAULT_LEASE_TTL_MS
        );
        assert_eq!(
            RUN_CEREMONY_STEP_LEASE_TTL_MS,
            RunCeremonyStepInput::DEFAULT_LEASE_TTL_MS
        );
        assert_eq!(
            CLAIM_CEREMONY_STEP_LEASE_TTL_MS,
            StartCeremonyStepInput::DEFAULT_LEASE_TTL_MS
        );
    }
}
