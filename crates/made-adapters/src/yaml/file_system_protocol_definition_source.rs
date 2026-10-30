use std::fs;
use std::path::{Path, PathBuf};

use async_trait::async_trait;
use made_core::entities::ProtocolDefinition;
use made_core::error::DomainError;
use made_core::ports::ProtocolDefinitionSourcePort;

use super::protocol_definition_yaml::ProtocolDefinitionYaml;

#[derive(Debug, Clone)]
pub struct FileSystemProtocolDefinitionSource {
    paths: Vec<PathBuf>,
}

impl FileSystemProtocolDefinitionSource {
    #[must_use]
    pub fn new(paths: impl IntoIterator<Item = PathBuf>) -> Self {
        let mut paths = paths.into_iter().collect::<Vec<_>>();
        paths.sort();
        Self { paths }
    }

    pub fn from_directory(path: impl AsRef<Path>) -> Result<Self, DomainError> {
        let mut paths = Vec::new();
        for entry in fs::read_dir(path).map_err(|_| DomainError::InvariantViolated {
            reason: "failed to read protocol yaml directory",
        })? {
            let entry = entry.map_err(|_| DomainError::InvariantViolated {
                reason: "failed to read protocol yaml directory entry",
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
impl ProtocolDefinitionSourcePort for FileSystemProtocolDefinitionSource {
    async fn load(&self) -> Result<Vec<ProtocolDefinition>, DomainError> {
        self.paths
            .iter()
            .map(ProtocolDefinitionYaml::parse_path)
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
    use made_core::ports::ProtocolDefinitionSourcePort;
    use made_core::value_objects::ProtocolName;

    const PROTOCOL: &str = r#"
version: "1.0"
name: "source_protocol"
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
        fs::write(&b, PROTOCOL.replace("source_protocol", "source_b")).unwrap();
        fs::write(&ignored, "not yaml").unwrap();
        fs::write(&a, PROTOCOL.replace("source_protocol", "source_a")).unwrap();

        let source = FileSystemProtocolDefinitionSource::from_directory(&dir).unwrap();
        let definitions = source.load().await.unwrap();

        assert_eq!(source.paths(), &[a, b]);
        assert_eq!(definitions.len(), 2);
        assert_eq!(
            definitions[0].name(),
            &ProtocolName::new("source_a").unwrap()
        );
        assert_eq!(
            definitions[1].name(),
            &ProtocolName::new("source_b").unwrap()
        );
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn missing_directory_is_domain_error() {
        let err = FileSystemProtocolDefinitionSource::from_directory(
            "/tmp/underpass-missing-protocol-dir",
        )
        .unwrap_err();

        assert!(matches!(err, DomainError::InvariantViolated { .. }));
    }

    #[tokio::test]
    async fn invalid_yaml_file_aborts_load() {
        let dir = create_temp_dir();
        fs::write(dir.join("bad.yaml"), "version: [").unwrap();
        let source = FileSystemProtocolDefinitionSource::from_directory(&dir).unwrap();

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
