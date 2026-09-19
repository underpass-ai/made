# Online backup and artifact maintenance

The installed `made` executable accepts a bounded JSON administrative request:

```sh
made maintenance --describe request.json
made maintenance --request request.json
```

`--describe` prints the action, scope and exact target digest without executing
the operation. `--request` authenticates the local host from trusted process
configuration and evaluates its durable policy before acting. Output includes
the authorization evidence and result; preserve it with the backup evidence.

Configure `MADE_AUTH_POLICY_ID`, `MADE_AUTH_TRUSTED_HOST_ID`,
`MADE_CEREMONY_STORE_PATH` and `MADE_ARTIFACT_STORE_PATH`. The stores and policy
must already exist. The request payload cannot choose its principal. Access to
this process configuration and the local store is a host trust boundary.
Never expose this local command directly as an unauthenticated web endpoint.

The host needs an explicit grant for the operation. Owning a policy does not
implicitly grant data-plane maintenance. Actions are `backup_store`,
`restore_store`, `garbage_collect_artifacts` and
`release_artifact_protection`. These installation-wide commands use global
scope. A configured separation rule additionally requires the exact approval;
provide its id as the optional top-level `approval_decision_id`.

## SQLite backup and isolated restore

```json
{
  "command": {
    "operation": "backup",
    "destination": "/srv/made-backups/checkpoint-001",
    "key": "backup:checkpoint-001"
  }
}
```

The online database snapshot includes journal, authorization, budgets and
receipts when they share the configured SQLite database. Artifact selection is
protected in the artifact store before capture, under its GC barrier. The
companion copy uses that exact selection. A backup-set manifest ties both
components together; protection is released only after both verify.

An interrupted backup retains its protection. Retry with the same destination
and key. An interruption that cannot safely recover its original database
boundary fails closed; explicitly abandon it and use a new key and destination.
A metadata-only retired artifact is identified as such, never represented as
having recoverable content.

```json
{
  "command": {
    "operation": "restore",
    "source": "/srv/made-backups/checkpoint-001",
    "destination": "/srv/made-restore/checkpoint-001"
  }
}
```

The destination must not exist. A completed restore contains `database.sqlite3`,
`artifacts/` and `restore-complete.json`. Start an isolated MADE instance with
those paths to inspect the recovered authority and journal. External Git/HTTP
effects are not reversed by restoring MADE's data. No point-in-time recovery
beyond the verified snapshot is claimed.

## Garbage collection

```json
{"command":{"operation":"gc_preview","retired_before":"2026-09-19T00:00:00Z"}}
```

The result contains `plan` and a preview. Submit that exact plan as the `plan`
object of a `gc_apply` request. Its contents, including the bounded lease, are
part of the authorization target. Applying rechecks live metadata, uploads and
protections under the same store lock before deleting bytes. An expired or
newly protected plan is refused; regenerate the preview. The result reports
actual bytes reclaimed, not a count substituted for bytes.

To abandon a failed backup protection, use `release_protection` with its exact
`key` and a nonempty `reason`. Receipt and restored-store protection namespaces
are refused by this command: retiring their live references needs a separate
reference-retirement workflow. Releasing protection does not itself delete
content or historical records.

The source policy retains the authorization decision. Save stdout for the
observed operation outcome as well; an authorization decision alone is not
evidence that a backup or deletion completed.

## PostgreSQL

With `MADE_POSTGRES_URL`, the same backup request captures a full `pg_dump`
archive. Journal, receipts, authorization, budget ledger and artifact bytes share
its database snapshot. Install compatible `pg_dump` and `pg_restore` clients in
the trusted host environment. Their paths are not controlled by a request.

Restore uses a pre-created empty database. Put its connection URL in a private
host variable whose name starts with `MADE_RESTORE_`, for example
`MADE_RESTORE_TARGET_URL`, and submit:

```json
{"command":{"operation":"restore_postgres","source":"/srv/made-backups/checkpoint-001","target_url_env":"MADE_RESTORE_TARGET_URL"}}
```

The resolved destination identity is included as a digest in authorization,
without emitting its credentials. The service rejects the original cluster/database
identity even through a different URL, and rejects a target containing user
tables. Inspect the result in that isolated database before any deployment change.

## Ambiguous external effects

`reconcile_execution` accepts a complete declared `ExecutionReceipt` as `receipt`.
It requires the separate `reconcile_execution_operation` action. The exact
request and configured store identity are sealed in the authorization target.
The persisted intent must match operation, request digest, producing fence and
connector. MADE also requires a durable ambiguity marker written by the real
connector execution or recovery path after it returned an unresolved outcome.
The connector's immutable recovery capability is not that marker: a queryable
HTTP, Git or OCI operation can become ambiguous, while an intent whose effect
was never attempted cannot be declared successful merely because its connector
supports manual reconciliation. This command never invokes the connector.

Supply at least one committed evidence artifact with the exact operation/receipt/
fence provenance. The service verifies and pins its content before recording the
receipt. Unsupported budget measurements are refused rather than silently
accepted as observed usage. Identical declarations are idempotent; contradictory
results are rejected. Claiming success without artifact evidence is insufficient.

Before recording the receipt, MADE saves and protects an audit artifact containing
the authorized declaration and actor evidence. This is explicitly an attempt
record. The returned result separately identifies the terminal receipt and audit
artifact. An authorization decision or attempt artifact alone never proves a
successful reconciliation. The original intent remains available.
The ambiguity marker also remains available as evidence of why an authorized
operator declaration was required.

`release_protection` also refuses the `reconciliation-audit:` namespace. Audit
retirement requires its own reference-retirement workflow.
