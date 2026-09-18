# Verify a capability

A support claim needs an edition, a surface and a build.

1. Record the installed executable version and selected backend.
2. Inspect `tools/list` and `made_discover_capabilities` on that process.
3. Compare the capability with the [parity ledger](../architecture/parity.tsv)
   and [support matrix](support-matrix.md) for the matching source version.
4. Execute a representative operation with known inputs. For a step, verify
   its real handler/output; for persistence, close and reopen the store; for
   parity, compare both backends over the same session.

The plugin manifest, Cargo package, running binary and published marketplace
snapshot can be different versions during source testing. Record those facts
separately. The [development guide](../development/README.md) names the gates
that check the catalogue, behavior and packaging boundaries.
