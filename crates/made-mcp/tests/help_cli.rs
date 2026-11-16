use std::process::{Command, Stdio};

#[test]
fn help_prints_usage_without_starting_stdio() {
    let output = Command::new(env!("CARGO_BIN_EXE_made-mcp"))
        .arg("--help")
        .stdin(Stdio::null())
        .output()
        .expect("run made-mcp --help");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("help is UTF-8");
    for expected in [
        "Usage:",
        "MCP stdio mode",
        "--version",
        "migrate-store",
        "bootstrap-authorization",
        "approve-guard",
        "grant",
        "keygen",
        "public-key",
        "export-evidence",
        "verify-evidence",
    ] {
        assert!(stdout.contains(expected), "missing {expected} in {stdout}");
    }
    assert!(output.stderr.is_empty());
}
