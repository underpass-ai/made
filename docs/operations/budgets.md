# Shared ceremony budgets

A budget belongs to the root ceremony. Its descendants inherit the same account;
starting another worker or retrying a claim does not create another allowance.
The ledger reserves an estimate before the claim is admitted, then reconciles it
against the durable execution receipt. Receipt identity makes reconciliation
idempotent.

Set `budget_limits` on `StartPublishedCeremony` or
`made_start_published_ceremony`. At least one positive ceiling is required:

```json
{
  "budget_limits": {
    "tokens": 4000,
    "tool_calls": 10
  }
}
```

The other dimensions are `duration_micros` and `cost_micros`. Duration is
accumulated execution time, distinct from a ceremony's wall-clock deadlines.
Cost uses integer millionths of the declared three-letter `currency`. A cost
ceiling and currency must be declared together; MADE does not convert currencies.
An omitted ceiling leaves that dimension unlimited. An explicit zero is invalid;
it cannot silently remove a limit.

Supply all four measurements in `budget_reservation` when claiming a budgeted
step. Each measurement declares `quality`: `observed`, `estimated`, or `unknown`.
Observed and estimated require an explicit unsigned `amount`, including when
that amount is zero. Unknown has no measured amount and is refused for every
limited dimension. A known zero in an admission estimate means the host knows
that operation will not use that dimension; it is not a terminal observation.

Concurrent workers reserve through a compare-and-swap on the root ledger. A
loser cannot reserve beyond the shared balance. If pause or cancellation wins
the subsequent ceremony admission race, the reservation stays charged and
pending. This version does not release such reservations online: absence of a
claim during one read cannot prove a slow writer will never commit it.

Use `GetBudgetReport` / `made_get_budget_report` to inspect limits, reservations,
observed and estimated consumption, unconfirmed quantities, overruns, and the
available balance. `ListPendingBudgetReservations` /
`made_list_pending_budget_reservations` pages through unreconciled reservations
using the last reservation id, with a maximum page size of 500.

Unknown consumption retains its reserved amount as unconfirmed. Observed
consumption can exceed the estimate: the ledger records the actual overrun and
refuses all further admission. It does not discard a result or undo an external
effect that has already happened. Exhaustion itself neither cancels nor completes
the ceremony; admitted work drains under the lifecycle policy.

Use a durable ledger alongside durable ceremony and receipt stores. The in-memory
composition is intentionally ephemeral. Back up and restore the ledger together
with those stores; restoring only the ceremony journal would lose reservations.
Pending reservations and unknown usage are operational evidence, not billing
records or proof that no cost was incurred.
