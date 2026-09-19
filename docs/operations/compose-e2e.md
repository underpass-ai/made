# Run the Compose integration

```bash
make e2e-compose
```

The [runner script](../../scripts/ci/e2e-compose.sh) selects the available
Docker/Podman Compose path, builds the stack and runs the repository's E2E
scenarios. Inspect its prerequisites and selected compose file before running
it on a machine with other services bound to the same ports.

Each run creates a one-day test CA plus server and owner client certificates in
the repository's ignored `tmp/` directory. It maps the owner's certificate to
the `TrustedHost` principal, bootstraps the SQLite-backed authorization policy
before the service starts, and uses the public mTLS administration RPC to issue
an explicit grant for the operations exercised by the scenarios. MADE and the
runner both run as non-root. The script also prepares a writable transient
SQLite directory so the bootstrap process and service reopen the same policy.
The Compose fixture also owns a stable, public test store identity and cursor
HMAC key for the duration of the run. They exist only to exercise restart-safe
paginated search; production deployments must supply their own identity and
secret key.
The exit trap removes both scratch directories and the Compose resources.

`MADE_E2E_AUTH_DIR` and `MADE_E2E_STATE_DIR` may point at operator-managed
directories when investigating a failed run. Supplying either variable makes
the caller responsible for cleanup. These fixtures contain test private keys
and service state; they must not be reused outside this local integration.

These tests exercise mutual TLS, authorization administration, service,
transport, persistence and provider-compatible stub wiring. Real vLLM paths
have separate `e2e-provider-vllm`,
`e2e-council-vllm` and `e2e-mcp-council-vllm` targets and require an operator's
endpoint and model. A compiled adapter or stub response does not establish
real-provider behavior.

For chart behavior, use `make helm-lint` and the Kubernetes E2E target in the
[development guide](../development/README.md).
