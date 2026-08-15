# made-api

The published contract of the embedded
[MADE by Underpass](https://github.com/underpass-ai/made) engine — what a
consuming product is allowed to know.

Plain views, a capability report, an error vocabulary and one trait. No
domain types, no adapters, no storage. A consumer that compiles against
this crate alone can be developed against a stub and later pointed at any
implementation that honours the same contract.

## Versioned by meaning

`CONTRACT_VERSION` moves when the meaning of this surface changes, and it
is deliberately independent of the crate's release number. Two builds of
the same release can differ in the features they were compiled with, so a
consumer that inferred capabilities from a version string would find out
it was wrong mid-run.

Check `ApiCapabilities` at startup instead: it reports what the engine
you are actually holding can do.

## Vocabulary

These types speak the engine's own language — councils, ceremonies,
steps, guards, interventions. Nothing of a consuming product's vocabulary
appears here, and nothing of this vocabulary needs to leak into a
product: the mapping belongs at the consumer's own boundary (ADR-001 in
the repository).

## License

Apache-2.0.
