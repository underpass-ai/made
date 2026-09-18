# Security

Report sensitive vulnerabilities through
[GitHub private vulnerability reporting](https://github.com/underpass-ai/made/security/advisories/new).
If unavailable, use the private maintainer channel already established for
your deployment. Do not publish credentials or an exploit before coordinated
triage. Include the commit/release, deployment configuration, impact and a
minimal redacted reproduction.

MADE is pre-1.0. Maintainers prioritize fixes on `main` and the current
release work; there is no promised long-term support window for older lines
or historical experiments. Response is best-effort. An advisory should name
affected/fixed versions and any required operator action.

The boundary covers this engine, its public protocols, adapters, images and
chart. The host supplies external authority and the declared actor identity.
A human-kind field is not authentication, a claim is not permission, a journal
seal is not proof of an external fact, and a completion fence does not prevent
an already-running worker from causing an external effect.

Keep provider credentials, database DSNs and TLS keys in managed secrets.
Do not persist them in agent descriptors, outputs or repository examples.
Pin deployment images, configure TLS/mTLS across trust boundaries and retain
the chart's non-root/read-only posture. The minimal plaintext/no-op profile
is for controlled smoke environments.

For suspected exposure, revoke affected credentials, preserve safe evidence,
inspect the relevant system access logs and deploy the fixed version with
reviewed configuration. See [operations](docs/operations/README.md) for
persistence and deployment boundaries.
