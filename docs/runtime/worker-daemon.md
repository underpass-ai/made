# Ceremony worker daemon

The MADE service can install one recoverable ceremony worker in the same
process as the API. It is disabled by default. Set `MADE_WORKER_ENABLED=true`
and configure exactly one route: `oci`, `git`, or `http`.

The daemon discovers candidates from the durable ceremony index and filters the
step's `handler_config.attributes.connector` before reserving capacity or
claiming a lease. If that attribute is absent, the handler kind must equal the
connector id. The fixed connector ids are `oci`, `git`, and `http`. Every MADE
process that participates in the same local worker pool must use the same
capacity directory and limits.

## Required settings

| Setting | Meaning |
|---|---|
| `MADE_WORKER_ENABLED` | `true` installs and starts the daemon. |
| `MADE_WORKER_CONNECTOR` | `oci`, `git`, or `http`. |
| `MADE_WORKER_OWNER_ID` | Stable lease owner for this daemon instance; unique among live processes. |
| `MADE_WORKER_PRINCIPAL_ID` | Trusted-host principal present in the authorization policy. |
| `MADE_WORKER_CAPACITY_DIRECTORY` | Local filesystem directory shared by all worker processes. |
| `MADE_WORKER_OPERATION_ROOT` | Durable connector operation records, outside an OCI workspace. |

The authorization policy must grant the configured principal the explicit
`enforce_ceremony_deadlines` and `claim_ceremony_step` actions for ceremonies it
may run. Lease renewal is admitted as `renew_ceremony_step_lease`, derived from
the audited claim decision and current claim fence. Completion derives its own
accepted-work authorization. A role in the ceremony definition does not grant
worker permission.

## Admission and lease policy

| Setting | Default |
|---|---:|
| `MADE_WORKER_LEASE_TTL_MS` | `30000` |
| `MADE_WORKER_HEARTBEAT_MS` | `10000` |
| `MADE_WORKER_CLAIM_PAGE_LIMIT` | `50` |
| `MADE_WORKER_RECOVERY_PAGE_LIMIT` | `100` |
| `MADE_WORKER_MAX_PARALLEL` | `3` |
| `MADE_WORKER_MAX_PAGES_PER_TURN` | `4` |
| `MADE_WORKER_INITIAL_BACKOFF_MS` | `100` |
| `MADE_WORKER_MAX_BACKOFF_MS` | `2000` |
| `MADE_WORKER_CAPACITY_GLOBAL` | `MADE_WORKER_MAX_PARALLEL` |
| `MADE_WORKER_CAPACITY_PER_ROOT` | `1` |
| `MADE_WORKER_CAPACITY_PER_CONNECTOR` | `MADE_WORKER_MAX_PARALLEL` |
| `MADE_WORKER_CAPACITY_PER_PROVIDER` | `1` |

The heartbeat must be positive and shorter than the lease TTL. Capacity counts
active operation identities, survives process death, and reconciles its local
ledger against the ceremony journal. This is multiprocess coordination on one
shared filesystem, not distributed admission across hosts.

The scheduling policy is explicit and versioned. Configure
`MADE_WORKER_POLICY_VERSION`, `MADE_WORKER_PRIORITY`, `MADE_WORKER_WEIGHT`,
`MADE_WORKER_COST`, `MADE_WORKER_REQUESTED_CAPACITY`, and
`MADE_WORKER_SCHEDULER_CAPACITY`. Defaults are version `1`, priority `0`, and
unit weight, cost, request, and scheduler capacity. Admission decisions carry
the policy version for observation and audit correlation.

`MADE_WORKER_ROOT_POLICIES_JSON` can override those four scheduling attributes
for an exact ceremony root. It is a trusted startup document keyed by root id;
each entry must contain exactly `priority`, `weight`, `cost`, and
`requested_capacity`. For example:

```json
{
  "customer-a": {"priority": 5, "weight": 1, "cost": 1, "requested_capacity": 1},
  "customer-b": {"priority": 5, "weight": 3, "cost": 1, "requested_capacity": 1}
}
```

Invalid or partial policy stops startup. Roots absent from the document use
the global values. Priority chooses among roots that currently have enough
capacity; larger values run first. A selected root keeps its turn while its
deficit can pay for more work, including across daemon polls. Quantum is added
only when a new root turn begins. Every time another root begins a turn, a
waiting root gains one priority point, so finite priority differences do not
cause indefinite starvation. There is no fixed bound in admission count:
latency also depends on weight, cost, and available capacity. Weight continues
to set the WDRR quantum and proportional service; priority does not replace
weight.

Every scheduling and post-admission decision is appended as
JSON Lines to `admission-decisions.jsonl` in the capacity directory. Set
`MADE_WORKER_ADMISSION_LOG_PATH` to choose another path. Each record includes
the root, operation, priority, weight, cost, requested capacity, policy version,
sequence, and one of `admitted`, `backpressure`, `capacity`, `permission`,
`budget`, or `draining`. `capacity` means the process-shared capacity ledger
refused a reservation; `backpressure` means the versioned in-process scheduler
deferred it. Permission and budget reasons are emitted only by their typed
policy boundaries; storage and transport failures remain claim failures.

## Connector settings

All routes use `MADE_WORKER_CONNECTOR_TIMEOUT_MS`, default `300000`.

OCI additionally requires `MADE_WORKER_OCI_IMAGE` pinned by digest and
`MADE_WORKER_OCI_WORKSPACE`. The optional policy settings are
`MADE_WORKER_OCI_NETWORK` (`none`), `MADE_WORKER_OCI_UID` (`65532`),
`MADE_WORKER_OCI_CPUS` (`1`), `MADE_WORKER_OCI_MEMORY_BYTES` (`536870912`),
`MADE_WORKER_OCI_PIDS` (`128`), and `MADE_WORKER_OCI_MAX_OUTPUT_BYTES`
(`1048576`). The image must contain `/usr/bin/timeout`; the connector uses it as
an in-container bound in addition to host cancellation.

Git requires `MADE_WORKER_GIT_REPOSITORY` pointing to the configured bare
repository and `MADE_WORKER_GIT_SCRATCH`. HTTP requires
`MADE_WORKER_HTTP_BASE`; it must be HTTPS, except loopback HTTP used for local
operation endpoints.

## Shutdown and recovery

Service shutdown calls `request_stop()` and awaits the daemon task. The host
stops admitting new pages, cancels a running connector when renewal loses
authority, and drains the accepted batch before returning. Startup scans the
receipt journal before new admission. Connector recovery uses the stable
operation id; an unqueryable external effect becomes reconciliation-required
and is never replayed blindly.

Budgeted steps use a host-bounded planner. Configure
`MADE_WORKER_BUDGET_POLICY_VERSION`, `MADE_WORKER_BUDGET_MAX_DURATION_MICROS`,
`MADE_WORKER_BUDGET_MAX_TOKENS`, `MADE_WORKER_BUDGET_MAX_COST_MICROS`, and
`MADE_WORKER_BUDGET_MAX_TOOL_CALLS` with non-negative ceilings; the policy
version must be positive. A zero cost ceiling explicitly permits only
zero-cost work. Each step then
declares the matching `budget_policy_version` plus numeric
`estimated_duration_micros`, `estimated_tokens`, `estimated_cost_micros`, and
`estimated_tool_calls` handler attributes. Missing, `unknown`, stale-version,
or over-ceiling estimates fail before claim. The planner never guesses tokens
or cost.

The budget policy settings are optional for a daemon that only handles
unbudgeted ceremonies. If all five are absent, unbudgeted work still runs and
every budgeted claim fails closed. Supplying only part of the policy is a
startup configuration error.
