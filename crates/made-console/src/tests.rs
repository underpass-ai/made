use clap::Parser;

use crate::{Args, ArtifactCommand, BudgetCommand, Command, OutputFormat};

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
}
