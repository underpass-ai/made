use made_client::v1::IssueAuthorizationGrantRequest;
use made_client::{MadeClient, MadeClientError};
use serde_json::json;

use crate::{authorization_render, render, AuthorizationCommand, OutputFormat};

pub async fn execute(
    client: &MadeClient,
    command: AuthorizationCommand,
    output: OutputFormat,
) -> Result<(), MadeClientError> {
    match command {
        AuthorizationCommand::Policy => {
            let policy = client.authorization_policy().await?;
            println!("{}", authorization_render::policy(&policy, output));
        }
        AuthorizationCommand::Issue {
            grant_id,
            grantee_id,
            actions,
            scope,
            valid_from,
            valid_until,
            delegation_depth,
            parent_grant_id,
        } => {
            let response = client
                .issue_authorization_grant(IssueAuthorizationGrantRequest {
                    grant_id,
                    grantee_id,
                    actions,
                    scope: Some(scope.into_proto()?),
                    valid_from: Some(parse_timestamp(&valid_from)?),
                    valid_until: valid_until.as_deref().map(parse_timestamp).transpose()?,
                    delegation_depth,
                    parent_grant_id,
                })
                .await?;
            print_mutation(response.version, response.existing, output);
        }
        AuthorizationCommand::Revoke { grant_id, reason } => {
            let response = client.revoke_authorization_grant(grant_id, reason).await?;
            print_mutation(response.version, response.existing, output);
        }
        AuthorizationCommand::Decisions { after, limit } => {
            let page = client.authorization_decisions(after, limit).await?;
            println!("{}", authorization_render::decisions(&page, output));
        }
    }
    Ok(())
}

fn print_mutation(version: u64, existing: bool, output: OutputFormat) {
    println!(
        "{}",
        render::message(&json!({"version": version, "existing": existing}), output)
    );
}

fn parse_timestamp(value: &str) -> Result<prost_types::Timestamp, MadeClientError> {
    let parsed = time::OffsetDateTime::parse(value, &time::format_description::well_known::Rfc3339)
        .map_err(|error| {
            MadeClientError::ProtocolViolation(format!("timestamp must be RFC3339: {error}"))
        })?;
    Ok(prost_types::Timestamp {
        seconds: parsed.unix_timestamp(),
        nanos: i32::try_from(parsed.nanosecond()).expect("timestamp nanos fit i32"),
    })
}
