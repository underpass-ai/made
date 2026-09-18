# Versioned snapshot envelope

Unreleased writers save ceremony snapshots with `snapshot_schema_version: 2`
and `folded_instance`. Current readers accept both this envelope and the older
`{version, instance}` envelope. Unknown versions and mixed envelopes are
rejected. Opening an old store does not rewrite its existing snapshots.

The field change is intentional: a 0.6.0 reader requires `instance`, so it
refuses new snapshots. Merely adding a version number would be insufficient
because the older deserializer ignores unknown fields. It could silently
discard new lifecycle state, then load a familiar tail event while skipping
earlier controls already included in the snapshot.

Saving a new snapshot therefore makes that ceremony incompatible with old
snapshot readers, even when its most recent event uses an old schema. Sealed
events and their hashes are unchanged. The v0.6.0 fixture tests verify old
data before and after a new claim and reopening.

Upgrade all readers before allowing new writers to append. Preserve a
consistent backup while writers are stopped. Replacing an executable with
0.6.0 is not a data rollback. Removing snapshots to make an old reader attempt
replay is not a supported downgrade procedure: the event stream may also
contain event schemas the old reader does not understand.
