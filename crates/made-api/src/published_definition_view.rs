use serde::{Deserialize, Serialize};

/// A definition that is now published, or already was.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublishedDefinitionView {
    pub name: String,
    pub version: String,
    pub digest: String,
    pub already_published: bool,
}
