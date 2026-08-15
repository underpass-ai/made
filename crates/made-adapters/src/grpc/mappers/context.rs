//! External context bundle: proto ↔ domain.

use made_core::entities::{ContextItem, ContextReference, ContextSummary, ExternalContextBundle};
use made_core::error::DomainError;
use made_proto::v1 as pb;

use super::attributes::attributes_from_struct;

pub fn external_context_from_proto(
    bundle: Option<pb::ExternalContextBundle>,
) -> Result<Option<ExternalContextBundle>, DomainError> {
    let Some(bundle) = bundle else {
        return Ok(None);
    };

    let summary = bundle.summary.map(summary_from_proto).transpose()?;
    let items = bundle
        .items
        .into_iter()
        .map(item_from_proto)
        .collect::<Result<Vec<_>, _>>()?;
    let references = bundle
        .references
        .into_iter()
        .map(reference_from_proto)
        .collect::<Result<Vec<_>, _>>()?;
    let metadata = attributes_from_struct(bundle.metadata)?;

    Ok(Some(ExternalContextBundle::new(
        bundle.bundle_id,
        bundle.schema_version,
        summary,
        items,
        references,
        metadata,
    )?))
}

fn summary_from_proto(summary: pb::ContextSummary) -> Result<ContextSummary, DomainError> {
    ContextSummary::new(summary.text, attributes_from_struct(summary.attributes)?)
}

fn item_from_proto(item: pb::ContextItem) -> Result<ContextItem, DomainError> {
    ContextItem::new(
        item.item_id,
        item.kind,
        item.title,
        if item.narrative.trim().is_empty() {
            None
        } else {
            Some(item.narrative)
        },
        attributes_from_struct(item.attributes)?,
        item.reference_ids,
    )
}

fn reference_from_proto(reference: pb::ContextReference) -> Result<ContextReference, DomainError> {
    ContextReference::new(
        reference.reference_id,
        reference.uri,
        if reference.title.trim().is_empty() {
            None
        } else {
            Some(reference.title)
        },
        if reference.media_type.trim().is_empty() {
            None
        } else {
            Some(reference.media_type)
        },
        attributes_from_struct(reference.attributes)?,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use prost_types::Struct as PbStruct;

    #[test]
    fn none_maps_to_none() {
        assert!(external_context_from_proto(None).unwrap().is_none());
    }

    #[test]
    fn typed_bundle_maps_to_domain_bundle() {
        let bundle = external_context_from_proto(Some(pb::ExternalContextBundle {
            bundle_id: "ctx-1".to_owned(),
            schema_version: "v1".to_owned(),
            summary: Some(pb::ContextSummary {
                text: "summary".to_owned(),
                attributes: Some(PbStruct::default()),
            }),
            items: vec![pb::ContextItem {
                item_id: "item-1".to_owned(),
                kind: "finding".to_owned(),
                title: "Observation".to_owned(),
                narrative: "narrative".to_owned(),
                attributes: Some(PbStruct::default()),
                reference_ids: vec!["ref-1".to_owned()],
            }],
            references: vec![pb::ContextReference {
                reference_id: "ref-1".to_owned(),
                uri: "s3://bucket/file.json".to_owned(),
                title: "evidence".to_owned(),
                media_type: "application/json".to_owned(),
                attributes: Some(PbStruct::default()),
            }],
            metadata: Some(PbStruct::default()),
        }))
        .unwrap()
        .unwrap();

        assert_eq!(bundle.bundle_id(), "ctx-1");
        assert_eq!(bundle.schema_version(), "v1");
        assert_eq!(bundle.items()[0].kind(), "finding");
        assert_eq!(bundle.references()[0].uri(), "s3://bucket/file.json");
    }
}
