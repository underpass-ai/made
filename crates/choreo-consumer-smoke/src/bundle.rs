//! Deterministic stub of the kernel rehydration bundle.
//!
//! **This is the seam where a real consumer would fetch a bundle
//! from Underpass KMP** (or any other context source) before calling
//! `RunCouncilDecision`. The smoke harness keeps that fetch out of
//! its dependency surface — adding a kernel client here would force
//! the binary to ship a kernel adapter, which couples Epic 12 to
//! Epic 2's runtime. Instead, the deterministic literal below stands
//! in for that fetch.
//!
//! A real consumer integration would replace this with:
//!
//! ```text
//!   let bundle = kernel.rehydrate(...).await?;
//!   harness.grpc.run_council_decision(req.with_external_context(bundle)).await
//! ```
//!
//! The smoke binary documents this in `docs/operations/consumer-smoke.md`.

use choreo_proto::v1 as pb;

const BUNDLE_ID: &str = "consumer-smoke-bundle-v1";
const SCHEMA_VERSION: &str = "v1";

/// Build a deterministic [`pb::ExternalContextBundle`] suitable for
/// Chain 1. Fields are stable across runs so an integration test can
/// pin assertions against them.
#[must_use]
pub fn deterministic_bundle() -> pb::ExternalContextBundle {
    pb::ExternalContextBundle {
        bundle_id: BUNDLE_ID.to_owned(),
        schema_version: SCHEMA_VERSION.to_owned(),
        summary: Some(pb::ContextSummary {
            text: "Deterministic stand-in for a real kernel rehydration bundle. \
                Used by Epic 12's consumer-smoke harness."
                .to_owned(),
            attributes: None,
        }),
        items: vec![pb::ContextItem {
            item_id: "stub-item-1".to_owned(),
            kind: "note".to_owned(),
            title: "Stub item".to_owned(),
            narrative: "This bundle is synthetic. The real consumer would replace it \
                with a kernel rehydration payload."
                .to_owned(),
            attributes: None,
            reference_ids: vec!["kernel-seam".to_owned()],
        }],
        references: vec![pb::ContextReference {
            reference_id: "kernel-seam".to_owned(),
            uri: "stub://consumer-smoke/seam".to_owned(),
            title: "kernel rehydration seam".to_owned(),
            media_type: "text/plain".to_owned(),
            attributes: None,
        }],
        metadata: None,
    }
}

/// Stable identifier for the bundle the harness ships. Tests pin
/// against it so a future shape change forces an explicit
/// confirmation.
#[must_use]
pub const fn bundle_id() -> &'static str {
    BUNDLE_ID
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deterministic_bundle_is_deterministic() {
        let a = deterministic_bundle();
        let b = deterministic_bundle();
        assert_eq!(a.bundle_id, b.bundle_id);
        assert_eq!(a.items.len(), b.items.len());
        assert_eq!(a.references.len(), b.references.len());
        assert_eq!(a.bundle_id, BUNDLE_ID);
    }

    #[test]
    fn references_carry_a_kernel_seam_label() {
        let bundle = deterministic_bundle();
        assert!(bundle
            .references
            .iter()
            .any(|r| r.reference_id == "kernel-seam"));
    }
}
