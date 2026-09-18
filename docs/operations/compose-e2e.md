# Run the Compose integration

```bash
make e2e-compose
```

The [runner script](../../scripts/ci/e2e-compose.sh) selects the available
Docker/Podman Compose path, builds the stack and runs the repository's E2E
scenarios. Inspect its prerequisites and selected compose file before running
it on a machine with other services bound to the same ports.

These tests exercise service, transport, persistence and provider-compatible
stub wiring. Real vLLM paths have separate `e2e-provider-vllm`,
`e2e-council-vllm` and `e2e-mcp-council-vllm` targets and require an operator's
endpoint and model. A compiled adapter or stub response does not establish
real-provider behavior.

For chart behavior, use `make helm-lint` and the Kubernetes E2E target in the
[development guide](../development/README.md).
