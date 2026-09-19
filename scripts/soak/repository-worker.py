#!/usr/bin/env python3
"""Reference external candidate for the recoverable worker soak harness."""

import json
import os
import pathlib
import sys
from datetime import datetime, timezone


def main() -> None:
    operation_id, request_digest, claim_fence, request_path, result_path = sys.argv[1:]
    result = pathlib.Path(result_path)
    operation_root = result.parent
    effects = operation_root / "effects"
    effects.mkdir(parents=True, exist_ok=True)

    invocation_log = operation_root / "invocations.log"
    with invocation_log.open("a", encoding="utf-8") as log:
        log.write(operation_id + "\n")
        log.flush()
        os.fsync(log.fileno())

    effect = effects / f"{operation_id}.json"
    temporary_effect = effects / f".{operation_id}.tmp"
    with temporary_effect.open("wb") as output:
        output.write(pathlib.Path(request_path).read_bytes())
        output.flush()
        os.fsync(output.fileno())
    os.replace(temporary_effect, effect)
    sync_directory(effects)

    payload = {
        "operation_id": operation_id,
        "request_digest": request_digest,
        "producer_claim_fence": claim_fence,
        "result": {"status": "COMPLETED", "output": {}},
        "observed_at": datetime.now(timezone.utc).isoformat(),
    }
    temporary_result = result.parent / f".{operation_id}.result.tmp"
    with temporary_result.open("w", encoding="utf-8") as output:
        json.dump(payload, output, separators=(",", ":"))
        output.flush()
        os.fsync(output.fileno())
    os.replace(temporary_result, result)
    sync_directory(result.parent)


def sync_directory(path: pathlib.Path) -> None:
    directory = os.open(path, os.O_RDONLY)
    try:
        os.fsync(directory)
    finally:
        os.close(directory)


if __name__ == "__main__":
    main()
