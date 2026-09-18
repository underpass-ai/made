# Security Policy

This repository is the security boundary for the Underpass
MADE: the gRPC API, MCP adapter, event contracts, provider
adapter wiring, container image, and Helm chart in this checkout.

MADE is product-agnostic and independently deployable. It
does not require KMP, PIR, Runtime, or any downstream product to run.
Security issues in sibling projects should be reported to those
projects unless the impact crosses into this repository's code,
protocols, images, or chart.

## Supported Versions

Until the first public stable release, security fixes are supported on:

- `main`;
- the current release-candidate branch, if one exists;
- the latest published container image and Helm chart for the current
  release candidate, when they exist.

Older experiments, historical design documents, local-only demo
profiles, and unpublished prototype branches are not maintained as
security-supported versions.

After the first stable release, this policy should be updated with an
explicit version matrix before older release lines are promised support.

## Reporting a Vulnerability

Do not open a public issue with exploit details, credentials, private
URLs, logs containing secrets, or proof-of-concept payloads that would
let another party reproduce the issue.

Preferred private channel:

- GitHub private vulnerability reporting:
  <https://github.com/underpass-ai/made/security/advisories/new>

If that channel is unavailable, contact the maintainers through the
private project channel already used for the deployment or customer
environment. Public issues may be used only for non-sensitive tracking
after maintainers have acknowledged the report and agreed on wording.

Include as much of this as you can safely share:

- affected commit, tag, image digest, or chart version;
- deployment profile (`values.minimal.yaml`, embedded NATS, Postgres,
  TLS/mTLS, Runtime executor, provider adapters, or custom values);
- the exact security impact and who can trigger it;
- minimal reproduction steps;
- relevant logs with secrets redacted;
- whether the issue is already exploited or only theoretical.

Until a formal commercial SLA exists, response is best-effort. Reports
with credible secret exposure, remote code execution, authentication
bypass, privilege escalation, or cross-tenant data exposure are treated
as release-blocking work before feature work.

## Coordinated Disclosure

Maintainers will avoid publishing exploit details before a fix,
mitigation, or clear non-impact conclusion is available. Reporters are
asked to keep the issue private during triage and fix preparation.

When a fix is released, the public advisory should include:

- affected versions or commits;
- fixed versions or image digests;
- required operator action, such as key rotation or chart upgrade;
- a concise impact statement;
- credit if the reporter wants it.

## Operational Security Baseline

The project tries to keep security claims tied to code, chart render
checks, or tests. Current baseline expectations:

- Pin production images by digest. The chart refuses an unpinned default
  image and refuses `latest` unless the development escape hatch is set.
- Keep provider credentials, Postgres DSNs, TLS keys, and client certs
  in a secret manager or Kubernetes Secret. Do not put credentials in
  agent descriptors, output contracts, values files, examples, Git
  history, or public issues.
- Enable gRPC TLS or mTLS for non-local deployments that cross trust
  boundaries. Plain gRPC profiles are for local smoke, isolated
  in-cluster development, or environments protected by another trusted
  transport layer.
- Use NetworkPolicy or equivalent controls to restrict inbound gRPC and
  HTTP health ports, and outbound DNS, NATS, Postgres, OTLP, Runtime,
  and provider endpoints to the minimum required set.
- Keep the default non-root container posture: read-only root
  filesystem, all Linux capabilities dropped, `RuntimeDefault` seccomp,
  and no mounted service account token unless a deployment has a
  specific reason to change it.
- Treat MCP clients as API clients. If MCP connects to a hardened
  MADE deployment, configure the MCP gRPC backend with the
  same TLS or mTLS posture.
- Provider adapters are enabled only when the binary is built with the
  relevant feature and the required environment variables are present.
  Startup logs expose provider kinds, not secret values.

## Secrets and Incident Containment

If a MADE deployment may have exposed secrets:

1. Revoke or rotate provider API keys and bearer tokens.
2. Rotate Postgres DSNs, NATS credentials embedded in URLs, TLS
   certificates, and MCP client certificates that could have been read.
3. Redeploy with a pinned fixed image digest and reviewed Helm values.
4. Check MADE logs, provider logs, Postgres access logs, NATS
   logs, and Kubernetes audit logs for unexpected access.
5. Re-register affected agents or contracts only after confirming they
   do not contain credentials.

Do not rely on agent descriptors as a secret store. Descriptors may be
persisted and are part of the API surface.

## What To Report

Useful reports include:

- secret leakage through logs, Debug output, events, MCP responses,
  gRPC responses, rendered manifests, or persisted descriptors;
- chart hardening regressions, such as dropped security contexts,
  missing TLS secret validation, accidental plaintext DSN rendering, or
  service account token mounting;
- bypasses of output-contract validation in Strict mode;
- unsafe provider credential handling;
- unauthenticated access paths in a deployment mode that claims TLS or
  mTLS protection;
- dependency vulnerabilities with a reachable path in this repository.

Expected behavior that is not a vulnerability by itself:

- the minimal and embedded-NATS profiles use plaintext in-cluster gRPC
  unless TLS is explicitly enabled;
- fixture, noop, and stub components are not production provider
  security boundaries;
- legacy PIR or KMP study documents are not deployment requirements for
  this repository.
