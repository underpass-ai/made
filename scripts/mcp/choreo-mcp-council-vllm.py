#!/usr/bin/env python3
"""Run the real-vLLM multi-agent council ceremony through MCP stdio.

The script intentionally drives the public MCP tool surface instead of
calling gRPC directly:

1. choreo_register_contract
2. choreo_register_agent, once per vLLM agent
3. choreo_create_council
4. choreo_run_council_decision

It then asserts that the MCP response carries the same peer-review
evidence as the gRPC E2E: multiple candidates, distinct authors,
schema-valid Report JSON, and revision_count > 0.
"""

from __future__ import annotations

import json
import os
import shlex
import subprocess
import sys
import time
from typing import Any


VLLM_ENDPOINT_ENV = "CHOREO_VLLM_ENDPOINT"
VLLM_MODEL_ENV = "CHOREO_VLLM_MODEL"
VLLM_AGENT_COUNT_ENV = "CHOREO_VLLM_AGENT_COUNT"
VLLM_MAX_TOKENS_ENV = "CHOREO_VLLM_MAX_TOKENS"
VLLM_TIMEOUT_SECS_ENV = "CHOREO_VLLM_TIMEOUT_SECS"
MCP_BIN_ENV = "CHOREO_MCP_BIN"
MCP_ENDPOINT_ENV = "CHOREO_MCP_GRPC_ENDPOINT"
MCP_TOOL_TIMEOUT_ENV = "CHOREO_MCP_TOOL_TIMEOUT_SECS"
RUN_ID_ENV = "CHOREO_E2E_RUN_ID"


class E2eError(RuntimeError):
    """User-facing assertion or invocation failure."""


def main(argv: list[str]) -> int:
    try:
        if argv == ["--self-test"]:
            validate_run_result(sample_result(), agent_count=3)
            print("MCP council vLLM E2E self-test passed")
            return 0
        if argv:
            raise E2eError("usage: choreo-mcp-council-vllm.py [--self-test]")
        run()
        return 0
    except E2eError as exc:
        print(f"error: {exc}", file=sys.stderr)
        return 1


def run() -> None:
    require_env(MCP_ENDPOINT_ENV)
    endpoint = require_env(VLLM_ENDPOINT_ENV)
    model = require_env(VLLM_MODEL_ENV)
    agent_count = env_u32(VLLM_AGENT_COUNT_ENV, 3)
    max_tokens = env_u32(VLLM_MAX_TOKENS_ENV, 512)
    timeout_secs = env_u32(VLLM_TIMEOUT_SECS_ENV, 300)
    tool_timeout_secs = env_u32(MCP_TOOL_TIMEOUT_ENV, max(600, timeout_secs * 2))
    if agent_count < 2:
        raise E2eError(f"{VLLM_AGENT_COUNT_ENV} must be >= 2")

    run_id = os.environ.get(RUN_ID_ENV, "").strip() or default_run_id()
    specialty = f"report-vllm-mcp-{run_id}"
    contract_id = f"scenario-mcp-report-vllm-real-{run_id}"
    command = mcp_command()

    print(
        "MCP council vLLM E2E starting: "
        f"specialty={specialty} model={model} agents={agent_count}"
    )

    call_tool(
        command,
        "choreo_register_contract",
        {
            "contract": {
                "contract_id": contract_id,
                "format": "json_object",
                "fields": {},
                "json_schema": json.dumps(report_schema(), separators=(",", ":")),
            }
        },
        request_id=1,
        timeout_secs=tool_timeout_secs,
    )
    print(f"registered contract: {contract_id}")

    for index in range(agent_count):
        agent_id = f"agent-{specialty}-{index}"
        call_tool(
            command,
            "choreo_register_agent",
            {
                "specialty": specialty,
                "agent": {
                    "agent_id": agent_id,
                    "specialty": specialty,
                    "kind": "vllm",
                },
                "agent_config": {
                    "provider.endpoint": endpoint,
                    "provider.model": model,
                    "provider.max_tokens": max_tokens,
                },
            },
            request_id=10 + index,
            timeout_secs=tool_timeout_secs,
        )
        print(f"registered agent: {agent_id}")

    call_tool(
        command,
        "choreo_create_council",
        {"specialty": specialty, "num_agents": agent_count},
        request_id=20,
        timeout_secs=tool_timeout_secs,
    )
    print(f"created council: {specialty}")

    result = call_tool(
        command,
        "choreo_run_council_decision",
        {
            "specialty": specialty,
            "contract_id": contract_id,
            "description": decision_description(run_id),
            "validation_mode": "VALIDATION_MODE_STRICT",
        },
        request_id=30,
        timeout_secs=tool_timeout_secs,
    )
    summary = validate_run_result(result, agent_count)
    print(
        "MCP council vLLM E2E passed: "
        f"task_id={summary['task_id']} "
        f"candidates_total={summary['candidates_total']} "
        f"candidates_passed={summary['candidates_passed']} "
        f"authors={summary['author_count']} "
        f"winner_revision_count={summary['winner_revision_count']}"
    )


def mcp_command() -> list[str]:
    raw = os.environ.get(MCP_BIN_ENV, "choreo-mcp").strip()
    if not raw:
        raise E2eError(f"{MCP_BIN_ENV} is empty")
    return shlex.split(raw)


def call_tool(
    command: list[str],
    name: str,
    arguments: dict[str, Any],
    request_id: int,
    timeout_secs: int,
) -> dict[str, Any]:
    request = {
        "jsonrpc": "2.0",
        "id": request_id,
        "method": "tools/call",
        "params": {"name": name, "arguments": arguments},
    }
    env = os.environ.copy()
    env["CHOREO_MCP_BACKEND"] = "grpc"
    try:
        proc = subprocess.run(
            command,
            input=json.dumps(request, separators=(",", ":")) + "\n",
            text=True,
            capture_output=True,
            timeout=timeout_secs,
            env=env,
            check=False,
        )
    except FileNotFoundError as exc:
        raise E2eError(f"{MCP_BIN_ENV} command not found: {command[0]}") from exc
    except subprocess.TimeoutExpired as exc:
        raise E2eError(f"{name} timed out after {timeout_secs}s") from exc

    if proc.returncode != 0:
        raise E2eError(
            f"{name} exited {proc.returncode}; stderr:\n{proc.stderr.strip()}"
        )

    lines = [line.strip() for line in proc.stdout.splitlines() if line.strip()]
    if not lines:
        raise E2eError(f"{name} returned no JSON-RPC response; stderr:\n{proc.stderr}")
    try:
        response = json.loads(lines[-1])
    except json.JSONDecodeError as exc:
        raise E2eError(f"{name} returned invalid JSON: {lines[-1]}") from exc

    if response.get("jsonrpc") != "2.0":
        raise E2eError(f"{name} returned non-JSON-RPC response: {response}")
    if "error" in response:
        raise E2eError(f"{name} JSON-RPC error: {response['error']}")

    result = expect_object(response.get("result"), f"{name}.result")
    if result.get("isError") is True:
        raise E2eError(f"{name} MCP tool error: {tool_error_text(result)}")
    return expect_object(result.get("structuredContent"), f"{name}.structuredContent")


def validate_run_result(result: dict[str, Any], agent_count: int) -> dict[str, int | str]:
    task_id = expect_str(result.get("task_id"), "task_id")
    validation = expect_object(result.get("validation"), "validation")
    if validation.get("passed") is not True:
        raise E2eError(f"validation.passed must be true: {validation}")
    candidates_total = expect_int(validation.get("candidates_total"), "candidates_total")
    candidates_passed = expect_int(validation.get("candidates_passed"), "candidates_passed")
    if candidates_total <= 1:
        raise E2eError(f"expected multiple candidates, got {candidates_total}")
    if candidates_passed < 1:
        raise E2eError("expected at least one schema-valid candidate")

    winner = expect_object(result.get("winner"), "winner")
    proposal = expect_object(winner.get("proposal"), "winner.proposal")
    winner_revision_count = expect_int(
        proposal.get("revision_count"), "winner.proposal.revision_count"
    )
    if winner_revision_count <= 0:
        raise E2eError("winner revision_count must be > 0")
    payload = parse_report_payload(expect_str(proposal.get("content"), "winner.proposal.content"))
    validate_report_payload(payload)

    candidates = expect_list(result.get("candidates"), "candidates")
    authors: set[str] = set()
    unrevised: list[str] = []
    for candidate in candidates:
        c = expect_object(candidate, "candidate")
        authors.add(expect_str(c.get("author_agent_id"), "candidate.author_agent_id"))
        revision_count = expect_int(c.get("revision_count"), "candidate.revision_count")
        if revision_count <= 0:
            unrevised.append(expect_str(c.get("proposal_id"), "candidate.proposal_id"))
    if len(authors) < min(agent_count, 2):
        raise E2eError(f"expected candidates from distinct agents, got {sorted(authors)}")
    if unrevised:
        raise E2eError(f"expected every candidate revised; unrevised={unrevised}")

    return {
        "task_id": task_id,
        "candidates_total": candidates_total,
        "candidates_passed": candidates_passed,
        "author_count": len(authors),
        "winner_revision_count": winner_revision_count,
    }


def parse_report_payload(raw: str) -> dict[str, Any]:
    try:
        value = json.loads(raw.strip())
    except json.JSONDecodeError as exc:
        raise E2eError(f"winner content must be JSON object; got: {raw}") from exc
    return expect_object(value, "winner report payload")


def validate_report_payload(payload: dict[str, Any]) -> None:
    required = {"report_id", "executive_summary", "findings", "recommended_actions"}
    if set(payload) != required:
        raise E2eError(
            f"report payload keys must be exactly {sorted(required)}, got {sorted(payload)}"
        )
    expect_nonempty_str(payload["report_id"], "report_id")
    expect_nonempty_str(payload["executive_summary"], "executive_summary")

    findings = expect_list(payload["findings"], "findings")
    if not findings:
        raise E2eError("findings must not be empty")
    for index, item in enumerate(findings):
        finding = expect_object(item, f"findings[{index}]")
        if set(finding) != {"id", "summary", "confidence"}:
            raise E2eError(f"findings[{index}] has invalid keys: {sorted(finding)}")
        expect_nonempty_str(finding["id"], f"findings[{index}].id")
        expect_nonempty_str(finding["summary"], f"findings[{index}].summary")
        confidence = expect_nonempty_str(finding["confidence"], f"findings[{index}].confidence")
        if confidence not in {"low", "medium", "high"}:
            raise E2eError(f"findings[{index}].confidence has invalid value {confidence!r}")

    actions = expect_list(payload["recommended_actions"], "recommended_actions")
    if not actions:
        raise E2eError("recommended_actions must not be empty")
    for index, item in enumerate(actions):
        action = expect_object(item, f"recommended_actions[{index}]")
        if set(action) != {"id", "summary", "approval_required"}:
            raise E2eError(f"recommended_actions[{index}] has invalid keys: {sorted(action)}")
        expect_nonempty_str(action["id"], f"recommended_actions[{index}].id")
        expect_nonempty_str(action["summary"], f"recommended_actions[{index}].summary")
        if not isinstance(action["approval_required"], bool):
            raise E2eError(f"recommended_actions[{index}].approval_required must be bool")


def decision_description(run_id: str) -> str:
    return (
        f"E2E run {run_id}: produce a concise incident-style Report JSON. "
        "Return only a JSON object, with no markdown and no explanatory text. "
        "Use exactly this minimal shape: "
        f'{{"report_id":"{run_id}","executive_summary":"one concise paragraph",'
        '"findings":[{"id":"finding-1","summary":"one concrete finding",'
        '"confidence":"medium"}],"recommended_actions":[{"id":"action-1",'
        '"summary":"one concrete action","approval_required":false}]}}. '
        "You may change string values to fit the task, but do not add top-level fields "
        "and do not add object fields that are not present in the JSON Schema."
    )


def report_schema() -> dict[str, Any]:
    return {
        "$schema": "http://json-schema.org/draft-07/schema#",
        "$id": "https://underpass.ai/choreographer/e2e/report-minimal.schema.json",
        "title": "Minimal Report Output Contract for real vLLM MCP E2E",
        "type": "object",
        "additionalProperties": False,
        "required": [
            "report_id",
            "executive_summary",
            "findings",
            "recommended_actions",
        ],
        "properties": {
            "report_id": {"type": "string", "minLength": 1, "maxLength": 128},
            "executive_summary": {"type": "string", "minLength": 1, "maxLength": 1024},
            "findings": {
                "type": "array",
                "minItems": 1,
                "maxItems": 3,
                "items": {
                    "type": "object",
                    "additionalProperties": False,
                    "required": ["id", "summary", "confidence"],
                    "properties": {
                        "id": {"type": "string", "minLength": 1, "maxLength": 64},
                        "summary": {"type": "string", "minLength": 1, "maxLength": 512},
                        "confidence": {"type": "string", "enum": ["low", "medium", "high"]},
                    },
                },
            },
            "recommended_actions": {
                "type": "array",
                "minItems": 1,
                "maxItems": 3,
                "items": {
                    "type": "object",
                    "additionalProperties": False,
                    "required": ["id", "summary", "approval_required"],
                    "properties": {
                        "id": {"type": "string", "minLength": 1, "maxLength": 64},
                        "summary": {"type": "string", "minLength": 1, "maxLength": 512},
                        "approval_required": {"type": "boolean"},
                    },
                },
            },
        },
    }


def sample_result() -> dict[str, Any]:
    content = json.dumps(
        {
            "report_id": "sample",
            "executive_summary": "short summary",
            "findings": [
                {"id": "finding-1", "summary": "sample finding", "confidence": "medium"}
            ],
            "recommended_actions": [
                {"id": "action-1", "summary": "sample action", "approval_required": False}
            ],
        },
        separators=(",", ":"),
    )
    return {
        "task_id": "task-sample",
        "winner": {
            "rank": 0,
            "proposal": {
                "proposal_id": "proposal-a",
                "author_agent_id": "agent-a",
                "content": content,
                "metadata": {},
                "revision_count": 1,
            },
            "validation": {"score": 1.0, "reports": []},
        },
        "validation": {"passed": True, "candidates_passed": 2, "candidates_total": 3},
        "candidates": [
            {
                "proposal_id": "proposal-a",
                "author_agent_id": "agent-a",
                "score": 1.0,
                "reports": [],
                "rank": 0,
                "passed": True,
                "revision_count": 1,
            },
            {
                "proposal_id": "proposal-b",
                "author_agent_id": "agent-b",
                "score": 0.9,
                "reports": [],
                "rank": 1,
                "passed": True,
                "revision_count": 1,
            },
            {
                "proposal_id": "proposal-c",
                "author_agent_id": "agent-c",
                "score": 0.8,
                "reports": [],
                "rank": 2,
                "passed": False,
                "revision_count": 1,
            },
        ],
    }


def tool_error_text(result: dict[str, Any]) -> str:
    content = result.get("content")
    if isinstance(content, list):
        texts = [
            item.get("text", "")
            for item in content
            if isinstance(item, dict) and isinstance(item.get("text"), str)
        ]
        if texts:
            return " ".join(texts)
    return json.dumps(result, sort_keys=True)


def require_env(name: str) -> str:
    value = os.environ.get(name, "").strip()
    if not value:
        raise E2eError(f"required env var {name} is not set")
    return value


def env_u32(name: str, default: int) -> int:
    raw = os.environ.get(name, "").strip()
    if not raw:
        return default
    try:
        value = int(raw)
    except ValueError as exc:
        raise E2eError(f"{name} must parse as u32") from exc
    if value < 0 or value > 2**32 - 1:
        raise E2eError(f"{name} must be a non-negative u32")
    return value


def default_run_id() -> str:
    return f"{os.getpid()}-{int(time.time() * 1000)}"


def expect_object(value: Any, name: str) -> dict[str, Any]:
    if not isinstance(value, dict):
        raise E2eError(f"{name} must be an object")
    return value


def expect_list(value: Any, name: str) -> list[Any]:
    if not isinstance(value, list):
        raise E2eError(f"{name} must be an array")
    return value


def expect_str(value: Any, name: str) -> str:
    if not isinstance(value, str):
        raise E2eError(f"{name} must be a string")
    return value


def expect_nonempty_str(value: Any, name: str) -> str:
    text = expect_str(value, name)
    if not text.strip():
        raise E2eError(f"{name} must not be empty")
    return text


def expect_int(value: Any, name: str) -> int:
    if isinstance(value, bool) or not isinstance(value, int):
        raise E2eError(f"{name} must be an integer")
    return value


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
