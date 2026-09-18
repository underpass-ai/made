# Operate MADE

For one host, start with [local SQLite and MCP](../embedded/README.md).
For a service, use the [Kubernetes guide](deploy-kubernetes.md). The two
compositions share the ceremony engine but expose different council surfaces.

- [Support matrix](support-matrix.md): checked capabilities and deployment inputs.
- [Observability](observability-runbook.md): health, metrics, traces and event evidence.
- [Consumer smoke](consumer-smoke.md): test the public service boundary.
- [Compose smoke](compose-e2e.md): local service integration.
- [Capability verification](capability-verification.md): distinguish source and running build.
- [Migrations](../migrations/README.md): client and durable-state changes.

Keep database backups, release identity, chart values and runtime configuration
traceable. A healthy process does not prove a ceremony did real work; inspect
the accepted outputs and their evidence.
