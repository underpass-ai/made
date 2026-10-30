//! Reading the published catalogue: proto ↔ application.
//!
//! The limit and the cursor are validated into their value objects
//! here, so a request that could not be honoured is refused before any
//! store is read rather than clamped into a different question.

use made_app::usecases::{PublishedProtocolDefinitionPage, PublishedProtocolDefinitionQuery};
use made_core::entities::{ProtocolCatalogueEntry, PublishedProtocolDefinition};
use made_core::error::DomainError;
use made_core::value_objects::{
    ProtocolDefinitionCursor, ProtocolDefinitionDigest, ProtocolDefinitionPageLimit, ProtocolName,
    ProtocolVersion,
};
use made_proto::v1 as pb;

use crate::yaml::PublishedProtocolDefinitionYaml;

use super::protocol_authoring::count;

pub fn published_protocol_definition_query_from_proto(
    request: pb::ListProtocolDefinitionsRequest,
) -> Result<PublishedProtocolDefinitionQuery, DomainError> {
    let protocol = (!request.protocol.trim().is_empty())
        .then(|| ProtocolName::new(request.protocol))
        .transpose()?;
    let limit = match request.limit {
        0 => ProtocolDefinitionPageLimit::default(),
        raw => ProtocolDefinitionPageLimit::new(u16::try_from(raw).map_err(|_| {
            DomainError::OutOfRange {
                field: "protocol_definition_page_limit",
                value: f64::from(raw),
                min: 1.0,
                max: f64::from(ProtocolDefinitionPageLimit::MAX),
            }
        })?)?,
    };
    let after = (!request.cursor.trim().is_empty())
        .then(|| ProtocolDefinitionCursor::parse(request.cursor.trim()))
        .transpose()?;
    Ok(PublishedProtocolDefinitionQuery::new(
        protocol, limit, after,
    ))
}

pub fn published_protocol_definition_identity_from_proto(
    request: &pb::GetProtocolDefinitionRequest,
) -> Result<(ProtocolName, ProtocolVersion), DomainError> {
    Ok((
        ProtocolName::new(request.protocol.as_str())?,
        ProtocolVersion::new(request.version.as_str())?,
    ))
}

pub fn list_protocol_definitions_response_from(
    page: &PublishedProtocolDefinitionPage,
) -> pb::ListProtocolDefinitionsResponse {
    pb::ListProtocolDefinitionsResponse {
        definitions: page.entries().iter().map(summary_from).collect(),
        next_cursor: page
            .next_cursor()
            .map(ToString::to_string)
            .unwrap_or_default(),
    }
}

/// The definition is written back as authoring YAML; a definition that
/// cannot be written without changing its digest is an error here, not
/// an approximation on the wire.
pub fn get_protocol_definition_response_from(
    published: &PublishedProtocolDefinition,
) -> Result<pb::GetProtocolDefinitionResponse, DomainError> {
    Ok(pb::GetProtocolDefinitionResponse {
        protocol: published.name().as_str().to_owned(),
        version: published.version().as_str().to_owned(),
        digest: published.digest().to_hex(),
        definition_yaml: PublishedProtocolDefinitionYaml::render(published)?,
    })
}

fn summary_from(entry: &ProtocolCatalogueEntry) -> pb::PublishedProtocolDefinitionSummary {
    match entry {
        ProtocolCatalogueEntry::Readable(published) => {
            let definition = published.definition();
            pb::PublishedProtocolDefinitionSummary {
                protocol: published.name().as_str().to_owned(),
                version: published.version().as_str().to_owned(),
                digest: published.digest().to_hex(),
                description: definition
                    .description()
                    .map(|description| description.as_str().to_owned())
                    .unwrap_or_default(),
                state_count: count(definition.states().len()),
                step_count: count(definition.steps().len()),
                unreadable: String::new(),
            }
        }
        ProtocolCatalogueEntry::Unreadable(unreadable) => pb::PublishedProtocolDefinitionSummary {
            protocol: unreadable.name().as_str().to_owned(),
            version: unreadable.version().as_str().to_owned(),
            digest: unreadable
                .recorded_digest()
                .map(ProtocolDefinitionDigest::to_hex)
                .unwrap_or_default(),
            description: String::new(),
            state_count: 0,
            step_count: 0,
            unreadable: unreadable.defect().to_string(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(protocol: &str, limit: u32, cursor: &str) -> pb::ListProtocolDefinitionsRequest {
        pb::ListProtocolDefinitionsRequest {
            protocol: protocol.to_owned(),
            limit,
            cursor: cursor.to_owned(),
        }
    }

    #[test]
    fn empty_fields_ask_for_the_first_default_page_of_everything() {
        let query = published_protocol_definition_query_from_proto(request("", 0, "")).unwrap();

        assert_eq!(query, PublishedProtocolDefinitionQuery::default());
    }

    #[test]
    fn a_name_a_limit_and_a_cursor_are_read_into_their_values() {
        let query =
            published_protocol_definition_query_from_proto(request("review", 2, "review@1.0"))
                .unwrap();

        assert_eq!(query.protocol().unwrap().as_str(), "review");
        assert_eq!(query.limit().get(), 2);
        assert_eq!(query.after().unwrap().to_string(), "review@1.0");
    }

    #[test]
    fn an_unusable_limit_or_cursor_is_refused_rather_than_clamped() {
        assert!(published_protocol_definition_query_from_proto(request("", 101, "")).is_err());
        assert!(published_protocol_definition_query_from_proto(request("", 70_000, "")).is_err());
        assert!(published_protocol_definition_query_from_proto(request("", 0, "review")).is_err());
        assert!(
            published_protocol_definition_query_from_proto(request("Bad Name", 0, "")).is_err()
        );
    }

    #[test]
    fn a_read_needs_both_halves_of_the_identity() {
        let read = |protocol: &str, version: &str| {
            published_protocol_definition_identity_from_proto(&pb::GetProtocolDefinitionRequest {
                protocol: protocol.to_owned(),
                version: version.to_owned(),
            })
        };

        assert!(read("review", "1.0").is_ok());
        assert!(read("", "1.0").is_err());
        assert!(read("review", "").is_err());
    }
}
