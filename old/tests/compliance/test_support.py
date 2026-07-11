from __future__ import annotations

import json
import unittest
from collections.abc import Iterator
from contextlib import contextmanager
from typing import Any

from tests.compliance.fixtures import FixtureWorkspace, workspace_from_fixture
from tests.compliance.mcp_client import MCPClient, MCPError


class ComplianceTestCase(unittest.TestCase):
    fixture_name = "tiny-js-project"

    def setUp(self) -> None:
        self.workspace_cm = workspace_from_fixture(self.fixture_name)
        self.workspace: FixtureWorkspace = self.workspace_cm.__enter__()
        self.client_cm = MCPClient(self.workspace.root)
        self.client = self.client_cm.__enter__()

    def tearDown(self) -> None:
        if hasattr(self, "client_cm"):
            self.client_cm.__exit__(None, None, None)
        if hasattr(self, "workspace_cm"):
            self.workspace_cm.__exit__(None, None, None)

    @contextmanager
    def session_for_fixture(self, fixture_name: str) -> Iterator[tuple[FixtureWorkspace, MCPClient]]:
        with workspace_from_fixture(fixture_name) as workspace:
            with MCPClient(workspace.root) as client:
                yield workspace, client

    def assert_tool_success(self, result: dict[str, Any]) -> dict[str, Any]:
        self.assertFalse(result.get("isError", False), f"expected tool success, got {result!r}")
        self.assertIn("content", result, f"MCP tool result must contain content: {result!r}")
        self.assertIsInstance(result["content"], list, f"content must be a list: {result!r}")
        return structured_payload(result)

    def assert_tool_error(self, tool_name: str, arguments: dict[str, Any]) -> dict[str, Any]:
        try:
            result = self.client.call_tool(tool_name, arguments)
        except MCPError as exc:
            self.assertIn("code", exc.error, f"JSON-RPC error must include code: {exc.error!r}")
            self.assertIn("message", exc.error, f"JSON-RPC error must include message: {exc.error!r}")
            return {"rpc_error": exc.error}
        self.assertTrue(result.get("isError", False), f"expected tool error for {tool_name}, got {result!r}")
        payload = structured_payload(result)
        self.assertTrue(payload or result.get("content"), f"tool error must be structured or contain content: {result!r}")
        return payload

    def assert_denied_or_permission_required(self, tool_name: str, arguments: dict[str, Any]) -> dict[str, Any]:
        payload = self.assert_tool_error(tool_name, arguments)
        blob = json.dumps(payload, sort_keys=True).lower()
        self.assertRegex(
            blob,
            r"(denied|forbidden|permission|outside|escape|unsafe|network|destructive|blocked)",
            f"error should explain denial or permission requirement: {payload!r}",
        )
        return payload

    def tool_text(self, result: dict[str, Any]) -> str:
        payload = structured_payload(result)
        for key in ("text", "content", "stdout", "diff", "preview"):
            value = payload.get(key)
            if isinstance(value, str):
                return value
        texts: list[str] = []
        for item in result.get("content", []):
            if isinstance(item, dict) and isinstance(item.get("text"), str):
                texts.append(item["text"])
        return "\n".join(texts)


def structured_payload(result: dict[str, Any]) -> dict[str, Any]:
    structured = result.get("structuredContent")
    if isinstance(structured, dict):
        return structured
    for item in result.get("content", []):
        if isinstance(item, dict) and isinstance(item.get("json"), dict):
            return item["json"]
        if isinstance(item, dict) and isinstance(item.get("text"), str):
            try:
                parsed = json.loads(item["text"])
            except json.JSONDecodeError:
                continue
            if isinstance(parsed, dict):
                return parsed
    return {}
