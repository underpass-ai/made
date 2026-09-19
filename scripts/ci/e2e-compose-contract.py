#!/usr/bin/env python3
"""Check that the protected Compose fixture can start fail-closed MADE."""

from __future__ import annotations

import argparse
import pathlib
import re
import sys
import tempfile


ROOT = pathlib.Path(__file__).resolve().parents[2]
DEFAULT_COMPOSE = ROOT / "tests/e2e/docker-compose.e2e.yaml"
REQUIRED_VALUES = {
    "MADE_GRPC_TLS_MODE": "mutual",
    "MADE_GRPC_TLS_CERT_PATH": "/etc/made/auth/server.pem",
    "MADE_GRPC_TLS_KEY_PATH": "/etc/made/auth/server.key",
    "MADE_GRPC_TLS_CLIENT_CA_PATH": "/etc/made/auth/ca.pem",
    "MADE_AUTH_POLICY_ID": "made-e2e-policy",
    "MADE_AUTH_MTLS_PRINCIPALS_PATH": "/etc/made/auth/principals.json",
    "MADE_CEREMONY_STORE_PATH": "/var/lib/made/made-e2e.sqlite3",
    "MADE_CEREMONY_STORE_ID": "made-e2e-store",
}
CURSOR_KEY = "MADE_CEREMONY_SEARCH_CURSOR_HMAC_KEY"


def made_environment(document: str) -> dict[str, str]:
    """Read the scalar environment entries of the top-level `made` service."""
    service = re.search(r"(?ms)^  made:\s*\n(?P<body>.*?)(?=^  [a-zA-Z0-9_-]+:\s*$)", document)
    if service is None:
        raise ValueError("Compose fixture has no top-level `made` service")
    environment = re.search(
        r"(?ms)^    environment:\s*\n(?P<body>.*?)(?=^    [a-zA-Z0-9_-]+:\s*)",
        service.group("body"),
    )
    if environment is None:
        raise ValueError("Compose fixture `made` service has no environment mapping")
    values: dict[str, str] = {}
    for key, quoted, bare in re.findall(
        r'^      ([A-Z][A-Z0-9_]+):\s*(?:"([^"]*)"|([^#\s]+))\s*(?:#.*)?$',
        environment.group("body"),
        re.MULTILINE,
    ):
        values[key] = quoted or bare
    return values


def validate(path: pathlib.Path) -> list[str]:
    environment = made_environment(path.read_text(encoding="utf-8"))
    errors = [
        f"{name} must be {expected!r} in the isolated E2E fixture"
        for name, expected in REQUIRED_VALUES.items()
        if environment.get(name) != expected
    ]
    cursor_key = environment.get(CURSOR_KEY)
    if cursor_key is None:
        errors.append(f"{CURSOR_KEY} is required in the isolated E2E fixture")
    elif re.fullmatch(r"[0-9a-f]{64}", cursor_key) is None:
        errors.append(f"{CURSOR_KEY} must be exactly 32 bytes encoded as lowercase hex")
    return errors


def assert_invalid(document: str, expected: str) -> None:
    scratch = ROOT / "tmp"
    scratch.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(
        prefix="made-compose-contract-", dir=scratch
    ) as directory:
        path = pathlib.Path(directory) / "compose.yaml"
        path.write_text(document, encoding="utf-8")
        errors = validate(path)
    if not any(expected in error for error in errors):
        raise AssertionError(f"expected {expected!r} in validation errors, got {errors!r}")


def self_test() -> None:
    document = DEFAULT_COMPOSE.read_text(encoding="utf-8")
    if errors := validate(DEFAULT_COMPOSE):
        raise AssertionError(f"canonical fixture is invalid: {errors!r}")
    for name in ("MADE_CEREMONY_STORE_ID", CURSOR_KEY):
        assert_invalid(re.sub(rf"^      {name}:.*\n", "", document, flags=re.MULTILINE), name)
    assert_invalid(
        re.sub(
            rf'(^      {CURSOR_KEY}:\s*)"[^"]+"',
            r'\1"not-hex"',
            document,
            flags=re.MULTILINE,
        ),
        "exactly 32 bytes",
    )


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("compose", nargs="?", type=pathlib.Path, default=DEFAULT_COMPOSE)
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        self_test()
    errors = validate(args.compose)
    if errors:
        for error in errors:
            print(f"e2e-compose contract: {error}", file=sys.stderr)
        return 1
    print(f"e2e-compose contract: OK ({args.compose})")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
