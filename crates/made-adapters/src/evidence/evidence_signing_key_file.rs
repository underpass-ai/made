use std::fs;
use std::io::Write as _;
use std::path::Path;

use made_core::error::DomainError;
use made_core::value_objects::{decode_hex, encode_hex};

/// The seed file: 32 bytes as 64 hex characters, owner-readable only.
///
/// Hex rather than raw bytes so a person can tell a key file from a
/// corrupt one at a glance, and so a backup copied through a text
/// channel survives.
pub(super) struct EvidenceSigningKeyFile;

impl EvidenceSigningKeyFile {
    pub(super) fn read(path: &Path) -> Result<[u8; 32], DomainError> {
        Self::require_private(path)?;
        let text = fs::read_to_string(path).map_err(|_| DomainError::InvariantViolated {
            reason: "evidence signing key file could not be read",
        })?;
        let bytes = decode_hex(text.trim(), "evidence_signing_key")?;
        bytes
            .try_into()
            .map_err(|_| DomainError::InvariantViolated {
                reason: "evidence signing key file does not hold a 32-byte seed",
            })
    }

    pub(super) fn write(path: &Path, seed: &[u8; 32]) -> Result<(), DomainError> {
        if path.exists() {
            return Err(DomainError::InvariantViolated {
                reason: "evidence signing key file already exists; rotate it on purpose",
            });
        }
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent).map_err(|_| DomainError::InvariantViolated {
                    reason: "evidence signing key directory could not be created",
                })?;
            }
        }
        let mut options = fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt as _;
            options.mode(0o600);
        }
        let mut file = options
            .open(path)
            .map_err(|_| DomainError::InvariantViolated {
                reason: "evidence signing key file could not be created",
            })?;
        writeln!(file, "{}", encode_hex(seed)).map_err(|_| DomainError::InvariantViolated {
            reason: "evidence signing key file could not be written",
        })?;
        Ok(())
    }

    #[cfg(unix)]
    fn require_private(path: &Path) -> Result<(), DomainError> {
        use std::os::unix::fs::PermissionsExt as _;
        let metadata = fs::metadata(path).map_err(|_| DomainError::InvariantViolated {
            reason: "evidence signing key file is missing",
        })?;
        if metadata.permissions().mode() & 0o077 != 0 {
            return Err(DomainError::InvariantViolated {
                reason: "evidence signing key file must be readable by its owner only (mode 600)",
            });
        }
        Ok(())
    }

    #[cfg(not(unix))]
    fn require_private(path: &Path) -> Result<(), DomainError> {
        if path.is_file() {
            Ok(())
        } else {
            Err(DomainError::InvariantViolated {
                reason: "evidence signing key file is missing",
            })
        }
    }
}
