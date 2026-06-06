SHELL := /usr/bin/env bash
.DEFAULT_GOAL := help

PROVIDER_FEATURES := --features choreo-adapters/agent-anthropic --features choreo-adapters/agent-openai --features choreo-adapters/agent-vllm
RUN_ARGS ?=
VERSION ?=

.PHONY: \
	help \
	contract fmt-check fmt clippy test bench-compile check \
	integration-nats integration-postgres integration \
	e2e-compose e2e-kubernetes e2e-provider-vllm e2e-council-vllm \
	consumer-smoke \
	helm-lint build-image build-provider-image \
	run run-otel \
	bench-trace bench-deliberate bench-experiment-001 bench-experiment-002 \
	version release

help:
	@printf '%s\n' \
		'Available targets:' \
		'  make check                  # contract + fmt-check + clippy + test + bench-compile' \
		'  make integration            # integration-nats + integration-postgres' \
		'  make e2e-compose            # manual E2E via docker/podman compose or podman-compose' \
		'  make e2e-kubernetes         # manual E2E via Kubernetes Job' \
		'  make e2e-provider-vllm      # provider-level vLLM E2E' \
		'  make e2e-council-vllm       # Choreographer council E2E against real vLLM' \
		'  make consumer-smoke         # drive the public RPC + bus surface as a consumer would' \
		'  make helm-lint              # helm lint + hardened render assertions' \
		'  make build-image            # production container image' \
		'  make build-provider-image   # provider E2E runner image' \
		'  make run RUN_ARGS="..."     # run choreographer locally' \
		'  make run-otel RUN_ARGS="..."' \
		'  make version VERSION=X.Y.Z' \
		'  make release VERSION=X.Y.Z'

contract:
	bash scripts/ci/contract-gate.sh

fmt-check:
	cargo fmt --all -- --check

fmt:
	cargo fmt --all

clippy:
	cargo clippy --workspace --all-targets --locked $(PROVIDER_FEATURES) -- -D warnings

test:
	cargo test --workspace --locked $(PROVIDER_FEATURES)

bench-compile:
	bash scripts/ci/bench-compile.sh

check: contract fmt-check clippy test bench-compile

integration-nats:
	bash scripts/ci/integration-nats.sh

integration-postgres:
	bash scripts/ci/integration-postgres.sh

integration: integration-nats integration-postgres

e2e-compose:
	bash scripts/ci/e2e-compose.sh

e2e-kubernetes:
	bash scripts/ci/e2e-kubernetes.sh

e2e-provider-vllm:
	bash scripts/ci/e2e-provider-vllm.sh

e2e-council-vllm:
	bash scripts/ci/e2e-council-vllm.sh

consumer-smoke:
	cargo run -p choreo-consumer-smoke -- \
		--endpoint $${CHOREOGRAPHER_ENDPOINT:-http://localhost:50055} \
		--chain $${CONSUMER_SMOKE_CHAIN:-all}

helm-lint:
	bash scripts/ci/helm-lint.sh

build-image:
	bash scripts/ci/container-image.sh

build-provider-image:
	bash scripts/ci/build-provider-image.sh

run:
	cargo run --locked -p choreo $(RUN_ARGS)

run-otel:
	cargo run --locked -p choreo --features otel $(RUN_ARGS)

bench-trace:
	cargo bench -p choreo-core --bench trace_context

bench-deliberate:
	cargo bench -p choreo-app --bench deliberate

bench-experiment-001:
	bash docs/experiments/001-baseline-deliberation-latency/run.sh

bench-experiment-002:
	bash docs/experiments/002-deliberation-scale-sweep/run.sh

version:
	@test -n "$(VERSION)" || { echo 'usage: make version VERSION=X.Y.Z' >&2; exit 1; }
	bash scripts/release.sh version "$(VERSION)"

release:
	@test -n "$(VERSION)" || { echo 'usage: make release VERSION=X.Y.Z' >&2; exit 1; }
	bash scripts/release.sh release "$(VERSION)"
