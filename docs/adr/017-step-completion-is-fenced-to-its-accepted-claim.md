# ADR-017: Step completion is fenced to its accepted claim

Status: Accepted (2026-09-18)

## Context

An expired worker could finish a replacement worker's attempt. Optimistic
stream revision checks did not prevent this: retry loaded the replacement
claim and sealed the old worker's result against it. This affected delegated
completion, incremental app execution and the one-shot driver (#127).

## Decision

`StepClaimFence` is a domain-owned, canonical 64-character lowercase SHA-256
value. Its domain-separated, length-delimited input binds the ceremony id,
step id, execution coordinates, lease owner, idempotency key and acquisition
and expiry instants. It is a concurrency identity, not an authorization token.

The accepted claim returns its fence before work starts. The aggregate requires
that same typed value on `ApplyStepResult` and checks it against the current
claim before deciding any completion, failure, context write or repetition.
A replacement claim invalidates the former fence. Lease expiry alone still
allows completion until a replacement is accepted. A second ending is refused.
App-owned execution captures the fence immediately after claim append and
retains it unchanged across handler execution, reloads and optimistic retries.

Direct proto adds `ClaimCeremonyStepResponse.claim_fence` and requires
`CompleteCeremonyStepRequest.claim_fence`. Both MCP editions return the same
field at the top level of the claim's structured content and require it on
completion. Empty, missing or malformed fences are invalid input; a well-formed
mismatching fence is a refused completion. No adapter supplies an omitted fence.
The Rust facade returns `StartCeremonyStepOutput`, containing the accepted
instance, attempt, stream version and fence. `CompleteCeremonyStepInput::new`
requires that typed fence as its fifth argument. Old Rust callers fail to
compile until migrated; old wire callers receive an explicit refusal.
Claim responses render that accepted instance and the audit identity at its
accepted stream version. A newer claim cannot replace the response snapshot or
its trace, correlation and causation fields while the original fence is returned.

The fence derives from already sealed claim data. This change adds no event or
snapshot field and does not alter an event schema version or historical hash.
Existing stores need no migration. A host recovering work that began before
this contract must first establish which accepted claim actually did that work;
it must never fetch a replacement claim and attach the old result to its fence.
A host that cannot establish ownership must let the lease expire and claim new
work with a new idempotency key.

## Verification

`made-embedded/tests/claim_fencing.rs` runs synchronized overlapping workers
against separate handles of one SQLite journal. Both delegated and app-owned
paths reject late A with no append, retain B's lease, and admit exactly one B
ending. Full fold, an intentionally retained replacement-claim snapshot plus
completion tail, reopen and the audit chain agree. The application retry test
injects a winning reclaim between decision and append and verifies refusal.
Direct RPC and both MCP integration sessions exercise missing, malformed and
wrong identities without append, then complete with the returned fence.
Legacy fixture and fold suites continue to verify sealed-history compatibility.
`made-tests-integration/tests/ceremony_claim_response.rs` pauses A after its
durable append, accepts B's replacement, then releases A's direct RPC response.
Both responses retain their own snapshot, audit identity and fence; the test
fails with the former current-head renderer.
