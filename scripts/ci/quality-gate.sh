#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

cd "${ROOT_DIR}"

provider_features=(
  --features made-adapters/agent-anthropic
  --features made-adapters/agent-openai
  --features made-adapters/agent-vllm
)

bash scripts/ci/contract-gate.sh
cargo fmt --all -- --check
bash scripts/ci/embedded-dependency-boundary.sh
cargo clippy -p made-mcp --all-targets --no-default-features --features embedded --locked -- -D warnings
cargo test -p made-mcp --all-targets --no-default-features --features embedded --locked
bash scripts/ci/made-plugin-smoke.sh
cargo clippy --workspace --all-targets --locked "${provider_features[@]}" -- -D warnings
cargo test --workspace --locked "${provider_features[@]}"
bash scripts/ci/bench-compile.sh
