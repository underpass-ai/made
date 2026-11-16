pub(super) fn print() -> i32 {
    println!("Usage: made-mcp [command]\n\nRun without arguments for MCP stdio mode.\n\nCommands:");
    println!("  --help                         Show this help");
    println!("  --version, -V, version         Show version");
    println!("  migrate-store <path>");
    println!("  bootstrap-authorization <store> --policy-id <id> --trusted-host-id <id>");
    println!("  {}", super::APPROVE_GUARD_USAGE);
    println!("  {}", made_mcp::GRANT_COMMAND);
    println!("  {}", super::KEYGEN_USAGE);
    println!("  public-key <key-file>");
    println!("  {}", super::EXPORT_EVIDENCE_USAGE);
    println!("  {}", super::VERIFY_EVIDENCE_USAGE);
    0
}
