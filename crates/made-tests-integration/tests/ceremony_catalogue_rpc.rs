//! Reading the published catalogue back over gRPC.
//!
//! Publishing answers with a digest. What these prove is that the
//! digest can be turned back into what was published — a name, a
//! version and a document — by a caller that kept nothing else.

use made_proto::v1::made_service_client::MadeServiceClient;
use made_proto::v1::{
    GetCeremonyDefinitionRequest, ListCeremonyDefinitionsRequest, PublishCeremonyDefinitionRequest,
    PublishCeremonyDefinitionResponse,
};
use made_tests_integration::grpc_fixture::GrpcFixture;
use tonic::transport::Channel;
use tonic::Code;

fn document(name: &str, version: &str) -> String {
    format!(
        r#"
version: "{version}"
name: "{name}"
description: "Read back from the catalogue"
states:
  - id: OPEN
    initial: true
  - id: DONE
    terminal: true
transitions:
  - from: OPEN
    to: DONE
    trigger: finish
steps:
  - id: work
    state: OPEN
    handler: noop
roles:
  - id: FACILITATOR
    allowed_actions:
      - work
      - finish
"#
    )
}

async fn publish(
    client: &mut MadeServiceClient<Channel>,
    name: &str,
    version: &str,
) -> PublishCeremonyDefinitionResponse {
    client
        .publish_ceremony_definition(PublishCeremonyDefinitionRequest {
            definition_yaml: document(name, version),
        })
        .await
        .expect("PublishCeremonyDefinition should succeed")
        .into_inner()
}

fn list(ceremony: &str, limit: u32, cursor: &str) -> ListCeremonyDefinitionsRequest {
    ListCeremonyDefinitionsRequest {
        ceremony: ceremony.to_owned(),
        limit,
        cursor: cursor.to_owned(),
    }
}

#[tokio::test]
async fn what_was_published_is_listed_in_order_and_pages_resume_after_the_cursor() {
    let fixture = GrpcFixture::start().await;
    let mut client = MadeServiceClient::new(fixture.channel);
    let published = publish(&mut client, "catalogue_beta", "1.0").await;
    publish(&mut client, "catalogue_alpha", "2.0").await;
    publish(&mut client, "catalogue_alpha", "1.0").await;

    let first = client
        .list_ceremony_definitions(list("", 2, ""))
        .await
        .expect("ListCeremonyDefinitions should succeed")
        .into_inner();
    let seen = first
        .definitions
        .iter()
        .map(|summary| format!("{}@{}", summary.ceremony, summary.version))
        .collect::<Vec<_>>();
    assert_eq!(seen, ["catalogue_alpha@1.0", "catalogue_alpha@2.0"]);
    assert_eq!(first.next_cursor, "catalogue_alpha@2.0");
    assert_eq!(
        first.definitions[0].description,
        "Read back from the catalogue"
    );
    assert_eq!(first.definitions[0].state_count, 2);
    assert_eq!(first.definitions[0].step_count, 1);

    let second = client
        .list_ceremony_definitions(list("", 2, &first.next_cursor))
        .await
        .expect("the second page should be served")
        .into_inner();
    assert_eq!(second.definitions.len(), 1);
    assert_eq!(second.definitions[0].ceremony, "catalogue_beta");
    assert_eq!(second.definitions[0].digest, published.digest);
    assert!(second.next_cursor.is_empty());

    let narrowed = client
        .list_ceremony_definitions(list("catalogue_beta", 0, ""))
        .await
        .expect("a name filter should be served")
        .into_inner();
    assert_eq!(narrowed.definitions.len(), 1);
}

#[tokio::test]
async fn a_published_version_reads_back_as_a_document_with_the_same_digest() {
    let fixture = GrpcFixture::start().await;
    let mut client = MadeServiceClient::new(fixture.channel);
    let published = publish(&mut client, "catalogue_read", "1.0").await;

    let read = client
        .get_ceremony_definition(GetCeremonyDefinitionRequest {
            ceremony: "catalogue_read".to_owned(),
            version: "1.0".to_owned(),
        })
        .await
        .expect("GetCeremonyDefinition should succeed")
        .into_inner();
    assert_eq!(read.ceremony, "catalogue_read");
    assert_eq!(read.version, "1.0");
    assert_eq!(read.digest, published.digest);

    // Offered again, the document it read back is the publication it
    // came from — not a near copy that would occupy the version.
    let again = client
        .publish_ceremony_definition(PublishCeremonyDefinitionRequest {
            definition_yaml: read.definition_yaml,
        })
        .await
        .expect("republishing the read document should be answered")
        .into_inner();
    assert_eq!(again.outcome, "already_published");
    assert_eq!(again.digest, published.digest);
}

#[tokio::test]
async fn an_unpublished_version_is_not_found_and_a_bad_request_is_refused() {
    let fixture = GrpcFixture::start().await;
    let mut client = MadeServiceClient::new(fixture.channel);

    let missing = client
        .get_ceremony_definition(GetCeremonyDefinitionRequest {
            ceremony: "never_published".to_owned(),
            version: "1.0".to_owned(),
        })
        .await
        .expect_err("an unpublished version has nothing to read");
    assert_eq!(missing.code(), Code::NotFound);

    let bad_cursor = client
        .list_ceremony_definitions(list("", 0, "no-separator"))
        .await
        .expect_err("a cursor that names nothing is refused");
    assert_eq!(bad_cursor.code(), Code::InvalidArgument);

    let bad_limit = client
        .list_ceremony_definitions(list("", 101, ""))
        .await
        .expect_err("a page larger than the bound is refused, not clamped");
    assert_eq!(bad_limit.code(), Code::InvalidArgument);
}
