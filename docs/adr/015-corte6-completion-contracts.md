# ADR 015 — C6 completion contracts

Status: accepted for implementation; acceptance remains open.

The original thirteen C6 fronts and four composite scenarios remain the scope.
The independent audit of `8adcf65a` is the baseline. A contract or fake is not
evidence of an integrated capability. Do not expand the architecture baseline.

## Execution and renewal

Workers select eligible candidates before consuming claims. WDRR is part of
that selection, followed by authoritative permission, budget and capacity
admission. Capacity is measured in active operations and coordinated across
processes for global, root and connector/provider limits. Durable reservations
are idempotent and recovered against journal claim state; a process-local
semaphore is insufficient.

Renewal preserves operation identity, attempt, budget reservation and accepted
claim fence. A separate durable renewal fact extends effective expiry without
replacing the producer identity. Renewal validates current ownership, effective
expiry and absolute deadlines under CAS. Old or expired owners cannot renew.
Pause closes new admission; accepted work drains according to existing C5 rules.
Cancellation/deadline does not authorize a new external effect.

The existing `CeremonyExecutionConnectorPort` is the worker/connector boundary.
An execution cancellation handle is explicit and must terminate the owned
execution group/container when continuation loses authority. Signature changes
require an A/B handoff and integration across every implementation.

## OCI and connectors

Acceptance targets Linux Docker OCI containers: non-root UID, read-only rootfs,
explicit workspace mounts, no capabilities, no-new-privileges, effective network
policy, CPU/memory/PID/output/time limits, image pinned by digest. This does not
claim a rootless Docker daemon or equivalent isolation on other platforms.
Security claims must be supported by real negative tests in this environment.

Git changes are scoped to an isolated acceptance repository. Requests bind the
expected ref/tip and change digest; ref update is conditional. Logical operation
identity must remain queryable after losing the response or receipt. HTTP uses
the same stable idempotency identity/digest and explicit operation lookup.
Unresolvable effects require authorized reconciliation with evidence, preserving
the original intent. No blind replay or external publication.

## Snapshot, backup and GC

Register durable protected blob references atomically with selection under the
same barrier consulted by GC. Backup pins do not expire merely because a paused
backup took longer than a lease. Release follows verified copied content or an
explicit abandonment. GC revalidates live metadata, receipt/snapshot/backup
references, uploads and restores before deletion and records actual bytes freed.

Snapshot journal, authorization, budgets and execution receipts at a consistent
database boundary. Protect every referenced blob before GC can invalidate that
snapshot; document lock ordering. SQLite and PostgreSQL restore only into isolated
targets. PITR is not claimed without a recoverable common WAL/blob horizon.
PostgreSQL restore takes a destination-database advisory lock on a dedicated
connection before identity and emptiness checks, retains it through
`pg_restore`, then rechecks while still fenced. A concurrent restore to that
same destination therefore observes the completed target and conflicts; a
different destination database has a distinct lock scope.

The SQLite recovery tests emit structured backend, digest, frontier and elapsed
time observations. They are acceptance evidence for the exercised fixture,
not an RPO, RTO or availability SLA.

## Ownership

| Role | Owned files/responsibilities |
|---|---|
| workers | made-app workers; core lease/renewal events and decisions; new capacity adapters and focused tests |
| execution | made-adapters execution/connectors; OCI and Git/HTTP tests |
| storage_ui (first wave) | made-adapters artifacts; artifact protection port; new SQLite/PostgreSQL backup adapters and tests |
| integrator | server composition, workspace Cargo, root exports, migrations, proto/gRPC/MCP/client/embedded, public authorization, shared CI/docs |

New capacity and backup adapters use distinct files. Coordinate root exports
and migrations with the integrator. Never format or rewrite another lane's files
or discard its changes. A later reviewer must not be the change's author.

## Acceptance resources

The host has ARM64 NVIDIA GB10, 121 GiB RAM, Docker, PostgreSQL 16 and NATS images,
kubectl and kind. Create a named isolated local acceptance cluster and registry;
never use an unrelated configured cluster. Two local provider profiles are fixed
in the execution evidence: official Qwen2.5 1.5B and 7B Instruct GGUF Q4_K_M,
with immutable revisions and file digests, served by the already available
llama.cpp image pinned by digest. External paid budget is zero. Model downloads
and real inference verification remain work for acceptance, not existing results.

Every front needs implementation, executed documentation and identified evidence.
Full existing gates, composite scenarios and independently reviewed results are
required before closure. Published tags are immutable; release/push is not an
implicit effect of this implementation ceremony.
