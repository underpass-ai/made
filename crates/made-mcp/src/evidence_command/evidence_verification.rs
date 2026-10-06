use made_core::value_objects::{CeremonyId, EvidenceBundleVerdict};

/// What `verify-evidence` found, for a reader with nothing but the file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvidenceVerification {
    ceremony_id: CeremonyId,
    head_version: u64,
    record_count: usize,
    public_key_hex: String,
    verdict: EvidenceBundleVerdict,
    /// `None` when no key was expected; otherwise whether the bundle's
    /// key is the expected one.
    key_matches: Option<bool>,
}

impl EvidenceVerification {
    #[must_use]
    pub const fn new(
        ceremony_id: CeremonyId,
        head_version: u64,
        record_count: usize,
        public_key_hex: String,
        verdict: EvidenceBundleVerdict,
        key_matches: Option<bool>,
    ) -> Self {
        Self {
            ceremony_id,
            head_version,
            record_count,
            public_key_hex,
            verdict,
            key_matches,
        }
    }

    #[must_use]
    pub const fn verdict(&self) -> EvidenceBundleVerdict {
        self.verdict
    }

    #[must_use]
    pub const fn key_matches(&self) -> Option<bool> {
        self.key_matches
    }

    /// Sound, and signed by the expected key when one was named.
    #[must_use]
    pub fn is_sound(&self) -> bool {
        self.verdict.is_sound() && self.key_matches != Some(false)
    }

    /// The verdict as the command prints it, one answer per line.
    #[must_use]
    pub fn lines(&self) -> Vec<String> {
        let mut lines = vec![
            format!(
                "ceremony `{}`: {} records through version {}",
                self.ceremony_id.as_str(),
                self.record_count,
                self.head_version
            ),
            format!(
                "chain: {}",
                match self.verdict.chain().defect() {
                    None => "intact".to_owned(),
                    Some(defect) => format!("broken ({defect:?})"),
                }
            ),
            format!(
                "head: {}",
                if self.verdict.head_matches() {
                    "the records end where the bundle says"
                } else {
                    "the records do not end where the bundle says"
                }
            ),
            format!(
                "signature: {} under public key {}",
                if self.verdict.signature_valid() {
                    "valid"
                } else {
                    "INVALID"
                },
                self.public_key_hex
            ),
        ];
        match self.key_matches {
            Some(true) => lines.push("key: the expected public key".to_owned()),
            Some(false) => lines.push("key: NOT the expected public key".to_owned()),
            None => lines.push(
                "key: no expected key was given; pass --public-key to pin the signer".to_owned(),
            ),
        }
        lines.push(if self.is_sound() {
            "verdict: sound".to_owned()
        } else {
            "verdict: NOT sound".to_owned()
        });
        lines
    }
}
