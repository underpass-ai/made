use std::path::PathBuf;

use made_core::value_objects::CeremonyId;

/// What `export-evidence` wrote and under which key, for the person who
/// will hand the file on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvidenceExportReceipt {
    ceremony_id: CeremonyId,
    head_version: u64,
    record_count: usize,
    public_key_hex: String,
    written_to: PathBuf,
}

impl EvidenceExportReceipt {
    #[must_use]
    pub const fn new(
        ceremony_id: CeremonyId,
        head_version: u64,
        record_count: usize,
        public_key_hex: String,
        written_to: PathBuf,
    ) -> Self {
        Self {
            ceremony_id,
            head_version,
            record_count,
            public_key_hex,
            written_to,
        }
    }

    #[must_use]
    pub fn ceremony_id(&self) -> &CeremonyId {
        &self.ceremony_id
    }

    #[must_use]
    pub const fn head_version(&self) -> u64 {
        self.head_version
    }

    #[must_use]
    pub const fn record_count(&self) -> usize {
        self.record_count
    }

    #[must_use]
    pub fn public_key_hex(&self) -> &str {
        &self.public_key_hex
    }

    #[must_use]
    pub fn written_to(&self) -> &PathBuf {
        &self.written_to
    }

    /// The receipt as the command prints it.
    #[must_use]
    pub fn lines(&self) -> Vec<String> {
        vec![
            format!(
                "exported ceremony `{}`: {} records through version {}",
                self.ceremony_id.as_str(),
                self.record_count,
                self.head_version
            ),
            format!("signed with public key {}", self.public_key_hex),
            format!("written to {}", self.written_to.display()),
            "verify anywhere with: made-mcp verify-evidence <file> --public-key <the key above>"
                .to_owned(),
        ]
    }
}
