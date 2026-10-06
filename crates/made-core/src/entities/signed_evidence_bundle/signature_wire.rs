use serde::{Deserialize, Serialize};

/// The attestation as the file spells it: an algorithm name and two hex
/// strings, so a person can read the key and a diff can show it.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SignatureWire {
    pub(super) algorithm: String,
    pub(super) public_key: String,
    pub(super) value: String,
}
