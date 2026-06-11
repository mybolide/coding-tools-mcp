from __future__ import annotations

import json
import re
import unittest
from pathlib import Path

from coding_tools_mcp.server import KILL_SESSION_STATUSES, TOOL_REGISTRY, input_schemas, tool_annotations
from tests.compliance.mcp_client import REQUIRED_TOOLS


ROOT = Path(__file__).resolve().parents[2]


class SchemaDriftTests(unittest.TestCase):
    def test_input_schemas_cover_exactly_the_registered_tools(self) -> None:
        self.assertEqual(set(input_schemas()), set(TOOL_REGISTRY))

    def test_profile_kill_session_status_enum_matches_live_constant(self) -> None:
        profile = (ROOT / "docs/profile-v0.1.md").read_text(encoding="utf-8")
        self.assertIn(json.dumps(list(KILL_SESSION_STATUSES)), profile)

    def test_profile_contains_every_live_tool_and_input_property(self) -> None:
        profile = (ROOT / "docs/profile-v0.1.md").read_text(encoding="utf-8")
        sections = markdown_tool_sections(profile)
        schemas = input_schemas()
        for tool_name in REQUIRED_TOOLS:
            with self.subTest(tool=tool_name):
                section = sections.get(tool_name, "")
                self.assertTrue(section, f"docs/profile-v0.1.md lacks section for {tool_name}")
                self.assertIn(tool_name, schemas, f"live input schema missing {tool_name}")
                for property_name in schemas[tool_name].get("properties", {}):
                    self.assertIn(f'"{property_name}"', section, f"{tool_name} profile missing {property_name}")

    def test_profile_contains_live_annotation_values(self) -> None:
        profile = (ROOT / "docs/profile-v0.1.md").read_text(encoding="utf-8")
        for tool_name in REQUIRED_TOOLS:
            annotations = tool_annotations(tool_name)
            for key, value in annotations.items():
                with self.subTest(tool=tool_name, annotation=key):
                    self.assertIn(str(key), profile)
                    self.assertIn(str(value).lower(), profile.lower())

    def test_tools_docs_list_matches_live_tool_names(self) -> None:
        text = (ROOT / "docs/tools-and-schemas.md").read_text(encoding="utf-8")
        missing = [tool for tool in REQUIRED_TOOLS if f"`{tool}`" not in text]
        self.assertEqual(missing, [])

    def test_profile_error_enum_contains_live_tool_failure_codes(self) -> None:
        source = (ROOT / "coding_tools_mcp/server.py").read_text(encoding="utf-8")
        profile = (ROOT / "docs/profile-v0.1.md").read_text(encoding="utf-8")
        codes = sorted(set(re.findall(r"ToolFailure\(\s*[\"']([A-Z_]+)[\"']", source)))
        missing = [code for code in codes if f'"{code}"' not in profile]
        self.assertEqual(missing, [])


def markdown_tool_sections(profile: str) -> dict[str, str]:
    sections: dict[str, str] = {}
    current_name: str | None = None
    current_lines: list[str] = []
    for line in profile.splitlines():
        if line.startswith("### "):
            if current_name is not None:
                sections[current_name] = "\n".join(current_lines)
            heading = line.removeprefix("### ").strip()
            current_name = heading.split()[0]
            current_lines = [line]
        elif current_name is not None:
            current_lines.append(line)
    if current_name is not None:
        sections[current_name] = "\n".join(current_lines)
    return sections
