from __future__ import annotations

from dataclasses import dataclass, field

from tests.compliance.test_support import ComplianceTestCase


@dataclass
class DeterministicMCPOnlyAgent:
    client: object
    calls: list[str] = field(default_factory=list)
    discoveries: int = 0

    def call(self, name: str, arguments: dict[str, object]) -> dict[str, object]:
        self.calls.append(name)
        return self.client.call_tool(name, arguments)  # type: ignore[attr-defined]

    def discover_tools(self) -> set[str]:
        self.discoveries += 1
        return {str(tool.get("name")) for tool in self.client.list_tools()}  # type: ignore[attr-defined]


class DogfoodMCPOnlyTests(ComplianceTestCase):
    def test_mcp_only_agent_completes_js_bugfix_without_direct_bypass(self) -> None:
        agent = DeterministicMCPOnlyAgent(self.client)
        required_loop_tools = {"search_text", "read_file", "apply_patch", "exec_command", "git_diff"}
        self.assertTrue(required_loop_tools <= agent.discover_tools())
        search = agent.call("search_text", {"query": "function add", "glob": "**/*.js"})
        self.assertIn("src/math.js", self.tool_text(search))
        source = agent.call("read_file", {"path": "src/math.js"})
        self.assertIn("return a - b", self.tool_text(source))
        patch = """*** Begin Patch
*** Update File: src/math.js
@@
 export function add(a, b) {
-  return a - b;
+  return a + b;
 }
*** End Patch
"""
        self.assert_tool_success(agent.call("apply_patch", {"patch": patch}))
        test = agent.call(
            "exec_command",
            {"cmd": "npm test", "timeout_ms": 20000, "yield_time_ms": 20000, "max_output_bytes": 20000},
        )
        self.assertEqual(self.assert_tool_success(test).get("exit_code"), 0)
        diff = agent.call("git_diff", {"path": "src/math.js"})
        self.assertIn("+  return a + b;", self.tool_text(diff))
        self.assertEqual(
            agent.calls,
            ["search_text", "read_file", "apply_patch", "exec_command", "git_diff"],
            "dogfood agent must use only MCP tools in the deterministic loop",
        )
        self.assertEqual(agent.discoveries, 1, "dogfood agent must retrieve the tool catalog before the loop")
