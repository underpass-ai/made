# Authorization in ceremony history

Protected mutations seal schema-3 audit records. `ReadCeremonyEvents`, the
positioned feed and live progress expose the exact admission evidence in
`CeremonyEventRecord.authorization`; both MCP backends render it as the
`authorization` object beside the ordinary record fields. Historical schema-1
and schema-2 records keep their previous fields and omit this object.

The evidence contains `decision_id`, `request_id`, `principal_id`, `action`,
`scope`, `target_digest`, `policy_version`, `admitted_at` and `valid_until`.
Timestamps preserve the precision used by the sealed record. The principal is
the identity admitted at the boundary; the business actor remains a separate
field. Changing either the sealed evidence or the event invalidates the hash.

An authorized policy administrator can resolve `decision_id` through the paged
`ListAuthorizationDecisions` API. That response carries the complete principal
(kind and authentication method), outcome, grant, and optional approval and
`accepted_work_decision_id` antecedents. Continuation therefore remains linked
to the admission that originally accepted the work, even if its grant has
subsequently been revoked.

An exact `CompleteCeremonyStep` retry recovers the accepted response from the
sealed journal, including its context writes and any reopened iteration. It
does not append another result or include work admitted after that response.
The claim fence, result, actor kind and original authenticated principal must
match. A different result for the same completed claim is refused.

Long-running `RunCeremony` and `RunCeremonyStep` calls renew admission evidence
before sealing a result after the initial decision expires. This completion
points to the decision sealed with its original claim and can drain after a
grant is revoked. A one-shot ceremony rechecks current grants before every new
claim; revocation stops the next step even while the initial decision remains
live. Other mutations after expiry require a current grant. The domain event
retains its observation time; the journal fact records its renewed admission
time when that is later, so authorization is never backdated.

Separation rules are exercised through `ApproveAuthorizationOperation` (or
`made_approve_authorization_operation`). The approver supplies the configured
approval action, execution action, authoritative scope, and the SHA-256 digest
of the exact execution request. The resulting decision records
`approved_action`; the executor sends its id in
`x-made-approval-decision-id`, or in MCP
`_meta.made_approval_decision_id`. Admission requires a live approval grant,
the configured action pair, the same target and covered scope, and a different
principal id. Reusing an approval for another target, action, or principal is
recorded as a denial before the domain operation runs.

Public history keeps its fields flat for transport consumers. To reconstruct a
schema-3 domain `AuditRecord` for independent verification, remove
`global_position` if present, move `authorization` to the envelope below, and
move all remaining fields except `schema_version` into `record`:

```json
{
  "schema_version": 3,
  "record": { "event_id": "...", "record_hash": [0] },
  "authorization": { "decision_id": "..." }
}
```

The example abbreviates both objects; reconstruction must preserve every
received field, timestamp and byte. Deserialize the complete envelope as
`AuditRecord`, then verify each digest and the chain. The stored v3 envelope is
intentionally different from the old flat storage format, so a v0.6 local
reader refuses unsupported records. No history is rewritten for transport.
