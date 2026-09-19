use clap::Parser;

use crate::{
    Args, ArtifactCommand, AuthorizationCommand, BudgetCommand, Command, OutputFormat,
    ReceiptCommand,
};

#[test]
fn watch_and_artifact_export_arguments_are_unambiguous() {
    let watch = Args::try_parse_from([
        "made-console",
        "--output",
        "json",
        "watch",
        "ceremony-1",
        "--cursor-file",
        "cursor.json",
        "--follow",
    ])
    .unwrap();
    assert_eq!(watch.output, OutputFormat::Json);
    assert!(matches!(
        watch.command,
        Command::Watch {
            ceremony_id,
            follow: true,
            ..
        } if ceremony_id == "ceremony-1"
    ));

    let export = Args::try_parse_from([
        "made-console",
        "artifact",
        "export",
        "artifact-1",
        "output.bin",
        "--force",
    ])
    .unwrap();
    assert!(matches!(
        export.command,
        Command::Artifact {
            command: ArtifactCommand::Export { force: true, .. }
        }
    ));
}

#[test]
fn budget_and_mtls_arguments_are_explicit() {
    let args = Args::try_parse_from([
        "made-console",
        "--endpoint",
        "https://made.example:50055",
        "--request-namespace",
        "request-7",
        "--tls-ca-certificate",
        "ca.pem",
        "--tls-client-certificate",
        "client.pem",
        "--tls-client-key",
        "client.key",
        "budget",
        "pending",
        "--limit",
        "25",
    ])
    .unwrap();
    assert_eq!(args.request_namespace.as_deref(), Some("request-7"));
    assert!(matches!(
        args.command,
        Command::Budget {
            command: BudgetCommand::Pending { limit: 25, .. }
        }
    ));

    assert!(Args::try_parse_from([
        "made-console",
        "--tls-client-certificate",
        "client.pem",
        "list",
    ])
    .is_err());
}

#[test]
fn bounded_arguments_are_rejected_by_clap() {
    assert!(
        Args::try_parse_from(["made-console", "tree", "ceremony-1", "--max-nodes", "1001",])
            .is_err()
    );
    assert!(Args::try_parse_from(["made-console", "artifact", "list", "--limit", "101",]).is_err());
    assert!(Args::try_parse_from(["made-console", "list", "--limit", "101"]).is_err());
}

#[test]
fn list_accepts_bounded_paging_and_lifecycle_filters() {
    let args = Args::try_parse_from([
        "made-console",
        "list",
        "--cursor",
        "opaque",
        "--limit",
        "25",
        "--id-prefix",
        "release_%",
        "--lifecycle",
        "paused",
    ])
    .unwrap();
    assert!(matches!(
        args.command,
        Command::List {
            cursor: Some(cursor),
            limit: 25,
            id_prefix: Some(prefix),
            lifecycle: Some(_),
        } if cursor == "opaque" && prefix == "release_%"
    ));
}

#[test]
fn receipt_recovery_is_explicitly_bounded_and_cursor_resumable() {
    let args = Args::try_parse_from([
        "made-console",
        "receipt",
        "recovery",
        "--after",
        "operation-42",
        "--limit",
        "25",
    ])
    .unwrap();
    assert!(matches!(
        args.command,
        Command::Receipt {
            command: ReceiptCommand::Recovery {
                after: Some(after),
                limit: 25,
            },
        } if after == "operation-42"
    ));
    assert!(
        Args::try_parse_from(["made-console", "receipt", "recovery", "--limit", "501",]).is_err()
    );
}

#[test]
fn authorization_issue_uses_typed_scope_without_a_principal_override() {
    let args = Args::try_parse_from([
        "made-console",
        "authorization",
        "issue",
        "grant-7",
        "worker-7",
        "--actions",
        "get_ceremony_instance,read_ceremony_events",
        "--scope",
        "ceremony-tree",
        "--scope-id",
        "root-7",
        "--valid-from",
        "2026-09-19T10:00:00Z",
    ])
    .unwrap();
    assert!(matches!(
        args.command,
        Command::Authorization {
            command: AuthorizationCommand::Issue { actions, scope, .. },
        } if actions.len() == 2 && scope.scope_id.as_deref() == Some("root-7")
    ));
}

#[test]
fn authorization_decision_pages_enforce_the_public_limit() {
    assert!(Args::try_parse_from([
        "made-console",
        "authorization",
        "decisions",
        "--limit",
        "501",
    ])
    .is_err());
}
