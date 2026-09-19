# Durable councils and imported snapshots

`EmbeddedMade::open(path)` persists agent descriptors, councils, contracts,
deliberations, statistics and the council journal in the configured SQLite
database. A custom builder remains explicitly ephemeral unless durable
adapters are selected. The council journal and named consumer positions are
independent of ceremony streams, hashes and cursors.

Postgres keeps its existing agent, council, deliberation and statistics
schemas. The new journal, contract and consumer tables are additive. Writes
lock a transactional ordinal before updating any council data, so a reader
cannot observe a later committed ordinal and subsequently miss an earlier
commit. Every registry mutation and its fact commit together.

## Data snapshots

`SqliteCouncilSnapshot` and `PostgresCouncilSnapshot` are offline operator
adapters for exporting and importing `CouncilDataSnapshot` JSON. The schema
version is 1. Supply a stable logical source label and the actual capture
time in `CouncilSnapshotProvenance`; never use a credential URL as a label.
The maximum encoded document is 32 MiB, with at most 100,000 rows per kind.
For larger datasets use the backend's consistent whole-store backup.

An import accepts an empty destination or data identical to its snapshot.
It atomically writes the data, retains the source snapshot and records one
`SnapshotImported` fact. It does not invent past phase changes, registration
commands or completions. Repeating the same source and content returns the
original import record. Reusing the source with different data, or importing
conflicting data into a destination, fails without any mutation. Exporting
existing Postgres rows and importing that identical snapshot on the same
store records their provenance without rewriting their history.

This data migration is distinct from restoring a backup: it deliberately
starts a new journal history and does not transfer active leases or consumer
positions. For continuation at the same positions, restore the complete store
and verify the journal and its separately stored blobs. Keep the source
snapshot as evidence outside the database.

Only supported non-secret provider settings are persisted: model, endpoint,
maximum tokens and timeout. Credentials remain in the host's provider factory
configuration. URLs containing user information, query parameters or fragments,
and unsupported attributes, are refused before import or registration. A live
agent object is insufficient for durable registration: supply its original
`AgentDescriptor` through `register_described`.

## Delivery and recovery

The public council journal service exposes bounded reads, cursor inspection,
lease, acknowledgement and release. Leases are issued by the store; expiry or
takeover fences an old consumer. Payload timestamps do not extend ownership.

Repeated publication of the same original `EventId` and payload produces one
record. Reusing an ID for a different fact is an error. Forwarding is at least
once: a lost delivery acknowledgement leaves the event pending. Consumers
must deduplicate the original event ID before applying effects. Successful
broker submission is not proof that an external consumer processed a message.
`PublishCouncilEventsUseCase` forwards only the existing five messaging event
kinds; configuration and imported snapshots advance its independent cursor
without being represented as fictitious bus events.

## Independent council consumers

`ReadCouncilEvents` reads records by council position without changing a
consumer. `GetCouncilEventCursor` returns its durable acknowledgement position.
`LeaseCouncilEvents` grants one exclusive lease for 1 ms to 1 hour; a busy
consumer receives no lease. After delivering records, use
`AcknowledgeCouncilEvents` with the complete returned lease and an existing
position. Acknowledgement releases that lease. `ReleaseCouncilEvents` releases
without advancing. Expired or replaced leases cannot acknowledge or release a
new owner's work. Council positions are never ceremony positions.

The embedded facade exposes the same five moves. Both MCP backends expose
`made_read_council_events`, `made_get_council_event_cursor`,
`made_lease_council_events`, `made_acknowledge_council_events` and
`made_release_council_events`. Read pages default to 200 records and are bounded
at 1000. Continue from `next_after`; an empty page means currently caught up.

The optional service publisher polls the durable journal even when no new event
or broker notification arrives. It forwards only the five original messaging
event types; registry and imported snapshot facts advance its cursor without
inventing bus events. Delivery is at least once. A lost acknowledgement can
cause a retry with the same original event ID, which downstream consumers must
deduplicate. A shutdown during delivery leaves the record unacknowledged; its
lease expires before another process can recover it.

An embedded host's explicit synchronous messaging callback still receives the
original event after the journal commits. The callback does not advance a
consumer cursor. A failed callback remains in the journal; hosts needing
durable external delivery should run `PublishCouncilEventsUseCase` as a leased
consumer instead of treating the callback as an exactly-once guarantee.
