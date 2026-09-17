//! The execution key this MCP server puts on a step when the caller
//! left the field out.
//!
//! `made-mcp:<uuid>` — one prefix, applied on every backend, before the
//! call leaves this layer. It is [`default_lease_owner_id`]'s sibling and
//! exists for the same reason: an omission must not mean a different
//! thing depending on which engine is behind the tool.
//!
//! An idempotency key is what a retry is recognised by, and it is
//! **sealed into the journal** — so an omitted one used to leave four
//! different spellings in the evidence of one session:
//! `made-mcp-{uuid}` for a step this server ran in process,
//! `made-mcp-external-{uuid}` for a claim, `grpc-{uuid}` and
//! `grpc-claim-{uuid}` for the same two calls over the wire, minted by
//! the server because this layer sent an empty string. Reading the
//! stream back told you which process happened to answer, which is not
//! something the journal should record about a call the client made the
//! same way twice.
//!
//! Unlike the lease owner, the backend is **not** in the key: a lease
//! owner answers "who is holding this", where knowing the kind of
//! process helps an operator; a key answers "is this the same execution
//! I already asked for", and the answer must not change because the
//! client was pointed somewhere else. The server keeps its own default
//! for clients that speak gRPC directly; from here it is never reached,
//! because this layer always sends a value.
//!
//! [`default_lease_owner_id`]: super::default_lease_owner::default_lease_owner_id

#[cfg(any(feature = "embedded", feature = "grpc"))]
use uuid::Uuid;

/// What every key this server mints starts with.
///
/// Gated on the two backends because a build with neither mints no
/// keys — and `uuid` is not even in the graph of such a build, so the
/// gate has to be the same one the function carries.
#[cfg(any(feature = "embedded", feature = "grpc"))]
pub(crate) const IDEMPOTENCY_KEY_PREFIX: &str = "made-mcp:";

#[cfg(any(feature = "embedded", feature = "grpc"))]
pub(crate) fn default_idempotency_key() -> String {
    format!("{IDEMPOTENCY_KEY_PREFIX}{}", Uuid::new_v4())
}

/// The rule in the words the tool schemas use, so the two descriptions
/// cannot drift from each other or from the code.
pub(crate) const DEFAULT_IDEMPOTENCY_KEY_RULE: &str =
    "Optional unique execution key, the key a retry of this exact call is \
     recognised by and the key sealed into the journal. Omitted, this MCP \
     server mints `made-mcp:<uuid>` before the call reaches the engine, so \
     the same omission is recorded the same way on every backend.";

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(any(feature = "embedded", feature = "grpc"))]
    #[test]
    fn one_prefix_and_a_fresh_key_each_time() {
        let first = default_idempotency_key();
        let second = default_idempotency_key();

        assert!(first.starts_with(IDEMPOTENCY_KEY_PREFIX), "{first}");
        assert!(second.starts_with(IDEMPOTENCY_KEY_PREFIX), "{second}");
        assert_ne!(
            first, second,
            "two calls are two executions, whatever they have in common"
        );
    }

    /// The key says nothing about which engine minted it. A key that
    /// did would make the journal of one session depend on where the
    /// client was pointed.
    #[cfg(any(feature = "embedded", feature = "grpc"))]
    #[test]
    fn the_backend_is_not_in_the_key() {
        let key = default_idempotency_key();
        assert!(!key.contains("embedded"), "{key}");
        assert!(!key.contains("grpc"), "{key}");
    }

    #[test]
    fn the_schemas_state_the_rule_they_implement() {
        assert!(DEFAULT_IDEMPOTENCY_KEY_RULE.contains("made-mcp:<uuid>"));
        assert!(DEFAULT_IDEMPOTENCY_KEY_RULE.contains("every backend"));
    }
}
