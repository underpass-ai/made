use std::fs;
use std::path::{Path, PathBuf};

/// The files SQLite keeps beside a store. A copy that took the
/// database and left the write-ahead log behind would be a copy of an
/// older store than the one on disk.
const SIDECARS: [&str; 2] = ["-wal", "-shm"];

/// Working copy of a ceremony store, and the two renames that install
/// it.
///
/// Copy-on-write, as ADR-008 did for the redb import: the operator's
/// file is never opened for writing, the migration runs entirely on a
/// copy, and the copy replaces the original only once every session it
/// imported has been proved to fold back. A failed run leaves the
/// original byte for byte, and a successful one leaves it beside the
/// new file under a name that says what it is.
#[derive(Debug)]
pub(super) struct WorkingCopy {
    original: PathBuf,
    working: PathBuf,
    backup: PathBuf,
}

impl WorkingCopy {
    /// Copy `original` to the working path, refusing rather than
    /// overwriting anything.
    ///
    /// Whether a backup already exists is [`Self::install`]'s
    /// question, not this one: a second run over a store that is
    /// already whole has nothing to install and must be able to say so
    /// rather than trip over the evidence the first run left.
    pub(super) fn create(original: &Path) -> Result<Self, String> {
        if !original.is_file() {
            return Err(format!("no ceremony store at {}", original.display()));
        }
        let copy = Self {
            original: original.to_path_buf(),
            working: suffixed(original, ".migrating"),
            backup: suffixed(original, ".pre-stream.backup"),
        };
        if copy.working.exists() {
            return Err(format!(
                "{} already exists: an earlier run left it behind; \
                 inspect it and remove it before migrating again",
                copy.working.display()
            ));
        }

        // `create_new` rather than `fs::copy`: the destination must not
        // exist, and a copy that would silently replace a file is the
        // one thing this protocol is for.
        fs::File::options()
            .write(true)
            .create_new(true)
            .open(&copy.working)
            .map_err(|error| format!("cannot create {}: {error}", copy.working.display()))?;
        copy_file(original, &copy.working)?;
        for sidecar in SIDECARS {
            let from = suffixed(original, sidecar);
            if from.is_file() {
                copy_file(&from, &suffixed(&copy.working, sidecar))?;
            }
        }
        Ok(copy)
    }

    pub(super) fn path(&self) -> &Path {
        &self.working
    }

    /// Put the migrated store where the original was, and keep the
    /// original beside it.
    ///
    /// Two renames rather than a copy, so the moment the new store
    /// becomes the store is a single filesystem operation and there is
    /// never a half-written file at the operator's path. An existing
    /// backup stops it before either rename: that file is the evidence
    /// of an earlier run, and it is never overwritten.
    pub(super) fn install(self) -> Result<PathBuf, String> {
        if self.backup.exists() {
            return Err(format!(
                "{} already exists: an earlier migration kept the original there, \
                 and it is never overwritten",
                self.backup.display()
            ));
        }
        rename_with_sidecars(&self.original, &self.backup)?;
        rename_with_sidecars(&self.working, &self.original).map_err(|error| {
            format!(
                "{error}. The original store is at {}; move it back to {}",
                self.backup.display(),
                self.original.display()
            )
        })?;
        Ok(self.backup)
    }

    /// Remove the working copy. Nothing else is touched, which is the
    /// whole answer for a run that did not install.
    pub(super) fn discard(self) -> Result<(), String> {
        for path in std::iter::once(self.working.clone())
            .chain(SIDECARS.iter().map(|s| suffixed(&self.working, s)))
        {
            if path.exists() {
                fs::remove_file(&path)
                    .map_err(|error| format!("cannot remove {}: {error}", path.display()))?;
            }
        }
        Ok(())
    }
}

fn copy_file(from: &Path, to: &Path) -> Result<(), String> {
    fs::copy(from, to).map(|_| ()).map_err(|error| {
        format!(
            "cannot copy {} to {}: {error}",
            from.display(),
            to.display()
        )
    })
}

fn suffixed(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.as_os_str().to_os_string();
    name.push(suffix);
    PathBuf::from(name)
}

fn rename_with_sidecars(from: &Path, to: &Path) -> Result<(), String> {
    fs::rename(from, to).map_err(|error| {
        format!(
            "cannot move {} to {}: {error}",
            from.display(),
            to.display()
        )
    })?;
    for sidecar in SIDECARS {
        let from = suffixed(from, sidecar);
        if from.is_file() {
            let to = suffixed(to, sidecar);
            fs::rename(&from, &to).map_err(|error| {
                format!(
                    "cannot move {} to {}: {error}",
                    from.display(),
                    to.display()
                )
            })?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store(directory: &Path, bytes: &[u8]) -> PathBuf {
        let path = directory.join("ceremonies.sqlite3");
        fs::write(&path, bytes).unwrap();
        path
    }

    #[test]
    fn a_missing_store_is_refused_by_name() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("absent.sqlite3");

        let error = WorkingCopy::create(&path).unwrap_err();

        assert!(error.contains("no ceremony store at"), "{error}");
        assert!(!suffixed(&path, ".migrating").exists());
    }

    #[test]
    fn the_copy_carries_the_write_ahead_log_too() {
        let directory = tempfile::tempdir().unwrap();
        let path = store(directory.path(), b"database");
        fs::write(suffixed(&path, "-wal"), b"pending").unwrap();

        let copy = WorkingCopy::create(&path).unwrap();

        assert_eq!(fs::read(copy.path()).unwrap(), b"database");
        assert_eq!(fs::read(suffixed(copy.path(), "-wal")).unwrap(), b"pending");
    }

    #[test]
    fn discarding_leaves_the_original_untouched() {
        let directory = tempfile::tempdir().unwrap();
        let path = store(directory.path(), b"database");
        let copy = WorkingCopy::create(&path).unwrap();
        let working = copy.path().to_path_buf();

        copy.discard().unwrap();

        assert!(!working.exists());
        assert_eq!(fs::read(&path).unwrap(), b"database");
    }

    #[test]
    fn installing_keeps_the_original_beside_the_new_store() {
        let directory = tempfile::tempdir().unwrap();
        let path = store(directory.path(), b"before");
        let copy = WorkingCopy::create(&path).unwrap();
        fs::write(copy.path(), b"after").unwrap();

        let backup = copy.install().unwrap();

        assert_eq!(fs::read(&path).unwrap(), b"after");
        assert_eq!(fs::read(&backup).unwrap(), b"before");
        assert_eq!(backup, suffixed(&path, ".pre-stream.backup"));
    }

    #[test]
    fn an_existing_backup_stops_an_install_from_overwriting_it() {
        let directory = tempfile::tempdir().unwrap();
        let path = store(directory.path(), b"database");
        fs::write(suffixed(&path, ".pre-stream.backup"), b"the original").unwrap();
        let copy = WorkingCopy::create(&path).unwrap();

        let error = copy.install().unwrap_err();

        assert!(error.contains("never overwritten"), "{error}");
        assert_eq!(
            fs::read(suffixed(&path, ".pre-stream.backup")).unwrap(),
            b"the original"
        );
        assert_eq!(fs::read(&path).unwrap(), b"database");
    }

    #[test]
    fn a_working_copy_left_by_an_earlier_run_is_refused() {
        let directory = tempfile::tempdir().unwrap();
        let path = store(directory.path(), b"database");
        fs::write(suffixed(&path, ".migrating"), b"half a migration").unwrap();

        let error = WorkingCopy::create(&path).unwrap_err();

        assert!(error.contains("inspect it"), "{error}");
        assert_eq!(
            fs::read(suffixed(&path, ".migrating")).unwrap(),
            b"half a migration"
        );
    }
}
