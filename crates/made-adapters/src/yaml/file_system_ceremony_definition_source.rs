use std::fs;
use std::path::{Path, PathBuf};

use async_trait::async_trait;
use made_core::entities::CeremonyDefinition;
use made_core::error::DomainError;
use made_core::ports::CeremonyDefinitionSourcePort;

use super::ceremony_definition_yaml::CeremonyDefinitionYaml;

#[derive(Debug, Clone)]
pub struct FileSystemCeremonyDefinitionSource {
    paths: Vec<PathBuf>,
}

impl FileSystemCeremonyDefinitionSource {
    #[must_use]
    pub fn new(paths: impl IntoIterator<Item = PathBuf>) -> Self {
        let mut paths = paths.into_iter().collect::<Vec<_>>();
        paths.sort();
        Self { paths }
    }

    pub fn from_directory(path: impl AsRef<Path>) -> Result<Self, DomainError> {
        let mut paths = Vec::new();
        for entry in fs::read_dir(path).map_err(|_| DomainError::InvariantViolated {
            reason: "failed to read ceremony yaml directory",
        })? {
            let entry = entry.map_err(|_| DomainError::InvariantViolated {
                reason: "failed to read ceremony yaml directory entry",
            })?;
            let path = entry.path();
            if is_yaml_path(&path) {
                paths.push(path);
            }
        }
        Ok(Self::new(paths))
    }

    #[must_use]
    pub fn paths(&self) -> &[PathBuf] {
        &self.paths
    }
}

#[async_trait]
impl CeremonyDefinitionSourcePort for FileSystemCeremonyDefinitionSource {
    async fn load(&self) -> Result<Vec<CeremonyDefinition>, DomainError> {
        self.paths
            .iter()
            .map(CeremonyDefinitionYaml::parse_path)
            .collect()
    }
}

fn is_yaml_path(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| matches!(extension, "yaml" | "yml"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use made_core::ports::CeremonyDefinitionSourcePort;
    use made_core::value_objects::CeremonyName;

    const CEREMONY: &str = r#"
version: "1.0"
name: "source_ceremony"
states:
  - id: STARTED
    initial: true
"#;

    #[tokio::test]
    async fn directory_source_loads_only_yaml_files_in_sorted_order() {
        let dir = create_temp_dir();
        let a = dir.join("a.yaml");
        let b = dir.join("b.yml");
        let ignored = dir.join("ignored.txt");
        fs::write(&b, CEREMONY.replace("source_ceremony", "source_b")).unwrap();
        fs::write(&ignored, "not yaml").unwrap();
        fs::write(&a, CEREMONY.replace("source_ceremony", "source_a")).unwrap();

        let source = FileSystemCeremonyDefinitionSource::from_directory(&dir).unwrap();
        let definitions = source.load().await.unwrap();

        assert_eq!(source.paths(), &[a, b]);
        assert_eq!(definitions.len(), 2);
        assert_eq!(
            definitions[0].name(),
            &CeremonyName::new("source_a").unwrap()
        );
        assert_eq!(
            definitions[1].name(),
            &CeremonyName::new("source_b").unwrap()
        );
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn missing_directory_is_domain_error() {
        let err = FileSystemCeremonyDefinitionSource::from_directory(
            "/tmp/underpass-missing-ceremony-dir",
        )
        .unwrap_err();

        assert!(matches!(err, DomainError::InvariantViolated { .. }));
    }

    #[tokio::test]
    async fn invalid_yaml_file_aborts_load() {
        let dir = create_temp_dir();
        fs::write(dir.join("bad.yaml"), "version: [").unwrap();
        let source = FileSystemCeremonyDefinitionSource::from_directory(&dir).unwrap();

        let err = source.load().await.unwrap_err();

        assert!(matches!(err, DomainError::InvariantViolated { .. }));
        fs::remove_dir_all(dir).unwrap();
    }

    fn create_temp_dir() -> PathBuf {
        let mut path = std::env::temp_dir();
        path.push(format!(
            "made-yaml-source-{}-{}",
            std::process::id(),
            time::OffsetDateTime::now_utc().unix_timestamp_nanos()
        ));
        fs::create_dir_all(&path).unwrap();
        path
    }
}
