/// A drawn topology and the same thing said in words.
///
/// Both, always. A picture is the fastest way to see a system and the
/// only way to see nothing at all if you cannot see it: the text
/// equivalent is not a fallback, it is the other half of the answer.
///
/// The source is Mermaid text. Turning it into an image is the host's
/// job — a renderer is not vendored here, because shipping a browser
/// engine to draw a box would cost more than the box is worth.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgenticSystemDiagram {
    mermaid: String,
    text_equivalent: Vec<String>,
}

impl AgenticSystemDiagram {
    #[must_use]
    pub fn new(mermaid: impl Into<String>, text_equivalent: Vec<String>) -> Self {
        Self {
            mermaid: mermaid.into(),
            text_equivalent,
        }
    }

    #[must_use]
    pub fn mermaid(&self) -> &str {
        &self.mermaid
    }

    #[must_use]
    pub fn text_equivalent(&self) -> &[String] {
        &self.text_equivalent
    }
}
