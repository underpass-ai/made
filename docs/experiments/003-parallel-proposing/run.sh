#!/usr/bin/env bash
set -euo pipefail
repo_dir=$(git rev-parse --show-toplevel)
evidence_dir=${1:?provide a new absolute evidence directory}
[[ "$evidence_dir" = /* ]] || { echo 'Evidence path must be absolute' >&2; exit 2; }
[[ ! -e "$evidence_dir" ]] || { echo 'Evidence directory already exists' >&2; exit 2; }
mkdir -p "$evidence_dir"
cd "$repo_dir"
git rev-parse HEAD > "$evidence_dir/commit.txt"
git status --porcelain > "$evidence_dir/worktree-status.txt"
rustc --version > "$evidence_dir/rustc.txt"
uname -a > "$evidence_dir/system.txt"
cargo test -p made-app experiment_003_bounded_proposing --locked -- --ignored --nocapture > "$evidence_dir/run.log" 2>&1
python3 - "$evidence_dir" <<'PY'
import sys
from pathlib import Path
directory = Path(sys.argv[1])
lines = [line for line in (directory / 'run.log').read_text().splitlines()
         if line[:1].isdigit() and len(line.split(',')) == 6]
assert len(lines) == 40, f'Expected 40 samples, got {len(lines)}'
(directory / 'measurements.csv').write_text(
    'provider_slots,width,repeat,elapsed_ms,peak_calls,completed\n' + '\n'.join(lines) + '\n')
PY
