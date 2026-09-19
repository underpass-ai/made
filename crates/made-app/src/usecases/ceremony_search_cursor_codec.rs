use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use hmac::{Hmac, Mac};
use made_core::error::DomainError;
use made_core::value_objects::{CeremonyId, CeremonyIdPrefix, CeremonyLifecyclePhase};
use sha2::Sha256;

use crate::usecases::{
    CeremonySearchCursor, CeremonySearchCursorKey, CeremonySearchCursorNamespace,
};

const DOMAIN: &[u8] = b"underpass.made.ceremony-search.cursor.v1\0";
const MAC_BYTES: usize = 32;
type HmacSha256 = Hmac<Sha256>;

/// Authenticates opaque keyset cursors and binds them to deployment and filters.
///
/// Cursor integrity never grants authority. Every page request must pass the
/// current authorization policy before this codec is used.
#[derive(Clone)]
pub struct CeremonySearchCursorCodec {
    key: CeremonySearchCursorKey,
    namespace: CeremonySearchCursorNamespace,
}

impl CeremonySearchCursorCodec {
    #[must_use]
    pub const fn new(
        key: CeremonySearchCursorKey,
        namespace: CeremonySearchCursorNamespace,
    ) -> Self {
        Self { key, namespace }
    }

    #[must_use]
    pub fn after(
        &self,
        ceremony_id: &CeremonyId,
        id_prefix: Option<&CeremonyIdPrefix>,
        lifecycle: Option<CeremonyLifecyclePhase>,
    ) -> CeremonySearchCursor {
        let payload = encode_payload(&self.namespace, ceremony_id, id_prefix, lifecycle);
        let mut mac = HmacSha256::new_from_slice(self.key.as_bytes())
            .expect("ceremony search cursor key has a fixed valid length");
        mac.update(&payload);
        let mut sealed = payload;
        sealed.extend_from_slice(&mac.finalize().into_bytes());
        CeremonySearchCursor::new(URL_SAFE_NO_PAD.encode(sealed))
    }

    pub fn decode(
        &self,
        cursor: &CeremonySearchCursor,
        id_prefix: Option<&CeremonyIdPrefix>,
        lifecycle: Option<CeremonyLifecyclePhase>,
    ) -> Result<CeremonyId, DomainError> {
        let sealed = URL_SAFE_NO_PAD
            .decode(cursor.as_str())
            .map_err(|_| invalid_cursor())?;
        if sealed.len() <= MAC_BYTES {
            return Err(invalid_cursor());
        }
        let (payload, observed_mac) = sealed.split_at(sealed.len() - MAC_BYTES);
        let mut mac = HmacSha256::new_from_slice(self.key.as_bytes())
            .expect("ceremony search cursor key has a fixed valid length");
        mac.update(payload);
        mac.verify_slice(observed_mac)
            .map_err(|_| invalid_cursor())?;
        decode_payload(payload, &self.namespace, id_prefix, lifecycle)
    }
}

impl std::fmt::Debug for CeremonySearchCursorCodec {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CeremonySearchCursorCodec")
            .finish_non_exhaustive()
    }
}

fn encode_payload(
    namespace: &CeremonySearchCursorNamespace,
    ceremony_id: &CeremonyId,
    id_prefix: Option<&CeremonyIdPrefix>,
    lifecycle: Option<CeremonyLifecyclePhase>,
) -> Vec<u8> {
    let mut payload = Vec::with_capacity(128);
    payload.extend_from_slice(DOMAIN);
    push_field(&mut payload, namespace.store_id().as_bytes());
    push_field(&mut payload, namespace.policy_id().as_bytes());
    push_field(&mut payload, ceremony_id.as_str().as_bytes());
    push_optional_field(
        &mut payload,
        id_prefix.map(|prefix| prefix.as_str().as_bytes()),
    );
    payload.push(lifecycle_code(lifecycle));
    payload
}

fn decode_payload(
    payload: &[u8],
    expected_namespace: &CeremonySearchCursorNamespace,
    expected_prefix: Option<&CeremonyIdPrefix>,
    expected_lifecycle: Option<CeremonyLifecyclePhase>,
) -> Result<CeremonyId, DomainError> {
    let mut remaining = payload.strip_prefix(DOMAIN).ok_or_else(invalid_cursor)?;
    let store_id = take_field(&mut remaining)?;
    let policy_id = take_field(&mut remaining)?;
    let ceremony_id = take_field(&mut remaining)?;
    let prefix = take_optional_field(&mut remaining)?;
    let Some((&lifecycle, trailing)) = remaining.split_first() else {
        return Err(invalid_cursor());
    };
    if !trailing.is_empty()
        || store_id != expected_namespace.store_id()
        || policy_id != expected_namespace.policy_id()
        || prefix.as_deref() != expected_prefix.map(CeremonyIdPrefix::as_str)
        || lifecycle != lifecycle_code(expected_lifecycle)
    {
        return Err(invalid_cursor());
    }
    CeremonyId::new(ceremony_id).map_err(|_| invalid_cursor())
}

fn push_field(target: &mut Vec<u8>, value: &[u8]) {
    target.extend_from_slice(&(value.len() as u32).to_be_bytes());
    target.extend_from_slice(value);
}

fn push_optional_field(target: &mut Vec<u8>, value: Option<&[u8]>) {
    match value {
        Some(value) => push_field(target, value),
        None => target.extend_from_slice(&u32::MAX.to_be_bytes()),
    }
}

fn take_field<'a>(remaining: &mut &'a [u8]) -> Result<&'a str, DomainError> {
    let length = take_length(remaining)?;
    if length == u32::MAX as usize || remaining.len() < length {
        return Err(invalid_cursor());
    }
    let (value, rest) = remaining.split_at(length);
    *remaining = rest;
    std::str::from_utf8(value).map_err(|_| invalid_cursor())
}

fn take_optional_field(remaining: &mut &[u8]) -> Result<Option<String>, DomainError> {
    let length = take_length(remaining)?;
    if length == u32::MAX as usize {
        return Ok(None);
    }
    if remaining.len() < length {
        return Err(invalid_cursor());
    }
    let (value, rest) = remaining.split_at(length);
    *remaining = rest;
    let value = std::str::from_utf8(value).map_err(|_| invalid_cursor())?;
    Ok(Some(value.to_owned()))
}

fn take_length(remaining: &mut &[u8]) -> Result<usize, DomainError> {
    let bytes: [u8; 4] = remaining
        .get(..4)
        .ok_or_else(invalid_cursor)?
        .try_into()
        .map_err(|_| invalid_cursor())?;
    *remaining = &remaining[4..];
    Ok(u32::from_be_bytes(bytes) as usize)
}

fn lifecycle_code(lifecycle: Option<CeremonyLifecyclePhase>) -> u8 {
    match lifecycle {
        None => 0,
        Some(CeremonyLifecyclePhase::Running) => 1,
        Some(CeremonyLifecyclePhase::Paused) => 2,
        Some(CeremonyLifecyclePhase::Ended) => 3,
    }
}

fn invalid_cursor() -> DomainError {
    DomainError::InvalidCharacters {
        field: "ceremony_search_cursor",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn codec(key: u8, store: &str, policy: &str) -> CeremonySearchCursorCodec {
        CeremonySearchCursorCodec::new(
            CeremonySearchCursorKey::new([key; 32]),
            CeremonySearchCursorNamespace::new(store, policy).unwrap(),
        )
    }

    #[test]
    fn cursor_is_authenticated_and_bound_to_filters() {
        let codec = codec(7, "store-a", "policy-a");
        let id = CeremonyId::new("ceremony-42").unwrap();
        let prefix = CeremonyIdPrefix::new("ceremony-").unwrap();
        let cursor = codec.after(&id, Some(&prefix), Some(CeremonyLifecyclePhase::Paused));
        assert_eq!(
            codec
                .decode(&cursor, Some(&prefix), Some(CeremonyLifecyclePhase::Paused))
                .unwrap(),
            id
        );
        assert!(codec
            .decode(&cursor, None, Some(CeremonyLifecyclePhase::Paused))
            .is_err());
        assert!(codec
            .decode(
                &cursor,
                Some(&prefix),
                Some(CeremonyLifecyclePhase::Running)
            )
            .is_err());

        let mut tampered = cursor.as_str().as_bytes().to_vec();
        let position = tampered.len() / 2;
        tampered[position] = if tampered[position] == b'A' {
            b'B'
        } else {
            b'A'
        };
        let tampered = CeremonySearchCursor::new(String::from_utf8(tampered).unwrap());
        assert!(codec
            .decode(
                &tampered,
                Some(&prefix),
                Some(CeremonyLifecyclePhase::Paused)
            )
            .is_err());
    }

    #[test]
    fn another_key_cannot_forge_the_cursor() {
        let id = CeremonyId::new("ceremony-42").unwrap();
        let cursor = codec(7, "store-a", "policy-a").after(&id, None, None);
        assert!(codec(8, "store-a", "policy-a")
            .decode(&cursor, None, None)
            .is_err());
    }

    #[test]
    fn another_replica_with_the_same_configuration_resumes_the_cursor() {
        let id = CeremonyId::new("ceremony-42").unwrap();
        let host_a = codec(7, "store-a", "policy-a");
        let host_b = codec(7, "store-a", "policy-a");
        let cursor = host_a.after(&id, None, None);
        assert_eq!(host_b.decode(&cursor, None, None).unwrap(), id);
    }

    #[test]
    fn namespace_prevents_cross_store_and_cross_policy_reuse() {
        let id = CeremonyId::new("ceremony-42").unwrap();
        let cursor = codec(7, "store-a", "policy-a").after(&id, None, None);
        assert!(codec(7, "store-b", "policy-a")
            .decode(&cursor, None, None)
            .is_err());
        assert!(codec(7, "store-a", "policy-b")
            .decode(&cursor, None, None)
            .is_err());
    }
}
