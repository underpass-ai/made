//! Composite keys for the canonical embedded store tables.
//!
//! `ceremony_id` is followed by a `0x00` separator and a big-endian
//! ordinal. Big-endian is what makes byte order match numeric order, so
//! a range scan returns a ceremony's records in the order they were
//! written. The separator is unambiguous by construction rather than by
//! convention: identifiers reject control characters, and `0x00` is
//! one, so no identifier can contain the byte that ends it.

use made_core::value_objects::{
    AgenticSystemId, CeremonyId, CeremonyName, CeremonyVersion, ExecutionOperationId, MemoryScope,
    StepClaimFence,
};

pub(super) const SEPARATOR: u8 = 0;
const ORDINAL_BYTES: usize = 8;

pub(super) fn scoped(ceremony_id: &CeremonyId, ordinal: u64) -> Vec<u8> {
    let id = ceremony_id.as_str().as_bytes();
    let mut key = Vec::with_capacity(id.len() + 1 + 8);
    key.extend_from_slice(id);
    key.push(SEPARATOR);
    key.extend_from_slice(&ordinal.to_be_bytes());
    key
}

/// The half-open byte range covering every record of one ceremony.
pub(super) fn scope_range(ceremony_id: &CeremonyId) -> (Vec<u8>, Vec<u8>) {
    (scoped(ceremony_id, 0), scoped(ceremony_id, u64::MAX))
}

/// The ceremony a scoped key belongs to, for scans that cross scopes.
///
/// Sliced by length, never by searching for the separator: the ordinal
/// is a big-endian `u64`, and small ordinals are mostly `0x00` bytes.
/// Looking for the last separator would split the key inside the
/// ordinal and report two records of one ceremony as belonging to two.
pub(super) fn ceremony_of(key: &[u8]) -> Option<&[u8]> {
    key.len()
        .checked_sub(ORDINAL_BYTES + 1)
        .map(|end| &key[..end])
}

/// Key of a row in the global event log: the position, big-endian, so a
/// range over `[from, from + limit)` walks the log in order.
pub(super) fn position(value: u64) -> [u8; ORDINAL_BYTES] {
    value.to_be_bytes()
}

/// One idempotent write inside a memory scope.
///
/// The scope is length-prefixed so neither colons in the scope nor arbitrary
/// bytes in a host-supplied idempotency key can move the boundary.
pub(super) fn memory_write(scope: &MemoryScope, idempotency_key: &str) -> Vec<u8> {
    let scope = scope.as_str().as_bytes();
    let mut key = Vec::with_capacity(2 + scope.len() + idempotency_key.len());
    key.extend_from_slice(&(scope.len() as u16).to_be_bytes());
    key.extend_from_slice(scope);
    key.extend_from_slice(idempotency_key.as_bytes());
    key
}

/// Inclusive range containing every write in one memory scope.
pub(super) fn memory_scope_range(scope: &MemoryScope) -> (Vec<u8>, Vec<u8>) {
    let start = memory_write(scope, "");
    let mut end = start.clone();
    end.push(u8::MAX);
    (start, end)
}

/// One technical claim of a semantic execution operation.
pub(super) fn execution_intent(
    operation_id: &ExecutionOperationId,
    claim_fence: &StepClaimFence,
) -> Vec<u8> {
    let operation = operation_id.as_str().as_bytes();
    let fence = claim_fence.as_str().as_bytes();
    let mut key = Vec::with_capacity(operation.len() + 1 + fence.len());
    key.extend_from_slice(operation);
    key.push(SEPARATOR);
    key.extend_from_slice(fence);
    key
}

/// Inclusive byte range containing every intent for one operation.
pub(super) fn execution_intent_range(operation_id: &ExecutionOperationId) -> (Vec<u8>, Vec<u8>) {
    let mut start = operation_id.as_str().as_bytes().to_vec();
    start.push(SEPARATOR);
    let mut end = start.clone();
    end.push(u8::MAX);
    (start, end)
}

/// Key for a published definition: the name length-prefixed, then the
/// version.
///
/// Length-prefixed rather than separated, because unlike a scoped
/// record this key needs no ordering — and a length prefix is
/// unambiguous without assuming anything about which characters a name
/// may contain.
pub(super) fn published(name: &CeremonyName, version: &CeremonyVersion) -> Vec<u8> {
    let name = name.as_str().as_bytes();
    let version = version.as_str().as_bytes();
    let mut key = Vec::with_capacity(2 + name.len() + version.len());
    key.extend_from_slice(&(name.len() as u16).to_be_bytes());
    key.extend_from_slice(name);
    key.extend_from_slice(version);
    key
}

/// Key of one row in the destination index.
///
/// A newline separates the two halves, and that is safe by construction
/// rather than by convention: destination keys and delivery identifiers
/// both refuse control characters, so neither half can contain the byte
/// that ends it.
pub(super) fn host_delivery_target(target_key: &str, delivery_id: &str) -> String {
    format!("{target_key}\n{delivery_id}")
}

/// The prefix covering every delivery addressed to one destination.
pub(super) fn host_delivery_target_prefix(target_key: &str) -> String {
    format!("{target_key}\n")
}

/// The prefix covering every delivery of one ceremony.
///
/// A delivery identifier opens with its ceremony and a colon, so the
/// prefix is exact for the ceremony it names. It narrows a scan; it
/// never decides membership, which the query itself still does.
pub(super) fn host_delivery_ceremony_prefix(ceremony_id: &CeremonyId) -> String {
    format!("{ceremony_id}:")
}

/// Key of one revision of one agentic system.
///
/// The revision is zero-padded to twenty digits so text order is
/// numeric order and a prefix scan walks a design's history from its
/// first revision to its head. A newline separates the halves, which
/// is safe by construction: identifiers refuse control characters, so
/// no identifier can contain the byte that ends it.
pub(super) fn agentic_system_revision(id: &AgenticSystemId, revision: u64) -> String {
    format!("{id}\n{revision:020}")
}

/// The prefix covering every revision of one agentic system.
pub(super) fn agentic_system_prefix(id: &AgenticSystemId) -> String {
    format!("{id}\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ceremony(raw: &str) -> CeremonyId {
        CeremonyId::new(raw).unwrap()
    }

    #[test]
    fn a_designs_revisions_walk_in_numeric_order() {
        let id = AgenticSystemId::new("delivery").unwrap();

        assert!(agentic_system_revision(&id, 2) < agentic_system_revision(&id, 10));
        assert!(agentic_system_revision(&id, 10) < agentic_system_revision(&id, u64::MAX));
        assert!(agentic_system_revision(&id, 1).starts_with(&agentic_system_prefix(&id)));
    }

    #[test]
    fn a_design_whose_id_extends_another_is_a_different_prefix() {
        let short = AgenticSystemId::new("delivery").unwrap();
        let long = AgenticSystemId::new("delivery-extra").unwrap();

        assert!(!agentic_system_revision(&long, 1).starts_with(&agentic_system_prefix(&short)));
    }

    #[test]
    fn byte_order_matches_numeric_order() {
        let id = ceremony("c1");

        assert!(scoped(&id, 2) < scoped(&id, 10));
        assert!(scoped(&id, 10) < scoped(&id, u64::MAX));
    }

    #[test]
    fn a_scope_range_covers_only_its_own_ceremony() {
        let (start, end) = scope_range(&ceremony("c1"));
        let neighbour = scoped(&ceremony("c2"), 1);
        let prefix_neighbour = scoped(&ceremony("c11"), 1);

        assert!(scoped(&ceremony("c1"), 7) >= start);
        assert!(scoped(&ceremony("c1"), 7) <= end);
        assert!(neighbour > end);
        assert!(
            prefix_neighbour > end,
            "a ceremony whose id extends another's must not fall inside its range"
        );
    }

    #[test]
    fn a_name_and_version_boundary_cannot_be_shifted() {
        // Without the length prefix, `plan` + `1.0` and `plan1` + `.0`
        // would produce the same bytes.
        let left = published(
            &CeremonyName::new("plan").unwrap(),
            &CeremonyVersion::new("1.0").unwrap(),
        );
        let right = published(
            &CeremonyName::new("plan1").unwrap(),
            &CeremonyVersion::new(".0").unwrap(),
        );

        assert_ne!(left, right);
    }

    #[test]
    fn position_keys_order_like_their_positions() {
        assert!(position(1) < position(2));
        assert!(position(255) < position(256));
        assert!(position(u64::MAX - 1) < position(u64::MAX));
    }

    #[test]
    fn a_memory_scope_range_excludes_prefix_neighbours() {
        let scope = MemoryScope::new("team:alpha").unwrap();
        let neighbour = MemoryScope::new("team:alphabet").unwrap();
        let (start, end) = memory_scope_range(&scope);

        assert!(memory_write(&scope, "first") >= start);
        assert!(memory_write(&scope, "last") <= end);
        assert!(memory_write(&neighbour, "first") > end);
    }

    #[test]
    fn the_ceremony_is_recoverable_from_a_scoped_key() {
        assert_eq!(ceremony_of(&scoped(&ceremony("c1"), 3)), Some(&b"c1"[..]));
    }

    #[test]
    fn an_ordinal_full_of_separator_bytes_does_not_split_the_key() {
        // A big-endian u64 below 2^8 is seven `0x00` bytes and one
        // payload byte. Anything that located the separator by
        // searching would cut the key inside the ordinal.
        for ordinal in [0, 1, 3, 255, u64::MAX] {
            assert_eq!(
                ceremony_of(&scoped(&ceremony("ceremony-1"), ordinal)),
                Some(&b"ceremony-1"[..]),
                "ordinal {ordinal} split the key in the wrong place"
            );
        }
    }
}
