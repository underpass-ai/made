# Operate MADE

For one host, start with [local SQLite and MCP](../embedded/README.md).
For a service, use the [Kubernetes guide](deploy-kubernetes.md). The two
compositions share the ceremony engine but expose different council surfaces.

- [Maintenance CLI](maintenance.md): authorized backup, isolated restore, GC and effect reconciliation.
- [Worker daemon](../runtime/worker-daemon.md): installed worker configuration and recovery.
- [Support matrix](support-matrix.md): checked capabilities and deployment inputs.
- [Observability](observability-runbook.md): health, metrics, traces and event evidence.
- [Consumer smoke](consumer-smoke.md): test the public service boundary.
- [Compose smoke](compose-e2e.md): local service integration.
- [Capability verification](capability-verification.md): distinguish source and running build.
- [Migrations](../migrations/README.md): client and durable-state changes.

Keep database backups, release identity, chart values and runtime configuration
traceable. A healthy process does not prove a ceremony did real work; inspect
the accepted outputs and their evidence.

## Database and artifact backup

SQLite stores the ceremony database and artifact blobs in separate files. The
maintenance backup first writes a durable artifact protection, captures the
database through SQLite's online backup API under the artifact barrier, then
copies exactly the protected blobs. `backup-set.json` binds the two component
manifests. The protection is released only after both components and the set
manifest verify. Lock order is artifact barrier, then SQLite read snapshot;
database writers never acquire the artifact barrier. Restore publishes a new
target directory only after its database and artifact store both verify.

PostgreSQL stores `artifact_blobs` in the same database as journals,
authorization state, budgets, receipts and artifact metadata. A custom-format
`pg_dump` therefore supplies one MVCC boundary for state and bytes; there is no
second filesystem blob copy. The runtime requires compatible `pg_dump` and
`pg_restore` clients (or explicitly configured pinned client paths). It hashes
the archive, validates its table of contents, and retains the durable
protection until the manifest verifies. Restore requires a different PostgreSQL
cluster/database identity and a database with no user tables. URL string
difference is not accepted as isolation evidence.

Protections do not expire by wall clock. Receipt and restore protections remain
live until an explicit authorized administrative release. A failed or
interrupted backup leaves its protection in place and resumes only when its
owner identity and immutable selection match. Retired artifact metadata whose
bytes were already collected is recorded as `retired_metadata_only`; backup and
restore preserve that history without claiming the absent bytes were copied.

Neither topology claims point-in-time recovery. Establishing a shared WAL and
external-blob horizon would be a separate operational contract.
