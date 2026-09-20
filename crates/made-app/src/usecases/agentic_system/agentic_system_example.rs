//! The worked example, packaged with the engine.
//!
//! A host that has the engine has the example: reading it should not
//! require having cloned the repository it lives in. The bytes are the
//! canonical file's, and the contract gate compares them, so the
//! document somebody reads in `api/examples` is the document the
//! engine hands out.

/// A system composing one published ceremony twice: once to deliver,
/// once to send the delivery back round for revision, at most twice.
///
/// The loop is what makes it worth shipping as an example. It is the
/// shape most real systems have — do the work, have it read, do it
/// again — and it is the shape that is easiest to write in a way that
/// could never finish.
pub const INTEGRATOR_DELIVERY_SYSTEM: &str =
    include_str!("examples/integrator-delivery-system.json");

#[cfg(test)]
mod tests {
    use time::OffsetDateTime;

    use super::*;
    use crate::usecases::agentic_system::AgenticSystemDesignDocument;

    /// The example is a document this engine accepts, not a document
    /// that merely looks like one. A shipped example that the decoder
    /// refuses would teach the wrong shape to everybody who copied it.
    #[test]
    fn the_shipped_example_is_a_document_the_decoder_accepts() {
        let document: AgenticSystemDesignDocument =
            serde_json::from_str(INTEGRATOR_DELIVERY_SYSTEM).expect("the example decodes");

        assert_eq!(document.id().as_str(), "integrator-delivery");
        assert_eq!(document.compositions().len(), 2);
        // Both compositions pin the same published definition, which
        // is the point: a system composes a ceremony more than once
        // for more than one purpose.
        for composition in document.compositions() {
            assert_eq!(composition.pin().name().as_str(), "integrator_delivery");
            assert!(composition
                .pin()
                .stated_digest()
                .expect("a readable pin")
                .is_none());
        }
    }

    /// Nothing is resolved yet, so the draft is buildable even though
    /// no ceremony has been published in this test.
    #[test]
    fn the_example_builds_a_draft_before_anything_is_published() {
        let document: AgenticSystemDesignDocument =
            serde_json::from_str(INTEGRATOR_DELIVERY_SYSTEM).expect("the example decodes");
        let digests = document
            .compositions()
            .iter()
            .map(|composition| {
                (
                    composition.id().clone(),
                    made_core::value_objects::CeremonyDefinitionDigest::from_bytes([0x11; 32]),
                )
            })
            .collect();

        let draft = document
            .into_draft(&digests, OffsetDateTime::UNIX_EPOCH)
            .expect("the example is a valid draft");

        // The loop's way back is cut, so the delivery can start at
        // all. Without that, a system that sends work round for
        // revision would be a system that never begins.
        let roots: Vec<&str> = draft
            .root_ceremonies()
            .iter()
            .map(|composition| composition.id().as_str())
            .collect();
        assert_eq!(roots, ["delivery"]);
    }
}
