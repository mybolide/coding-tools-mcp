#!/usr/bin/env python3
"""Smoke-check a running coding-tools-mcp HTTP server using the shared MCP client.

Usage:
    python scripts/mcp_smoke.py URL [--expect-permission-mode MODE] [CMD ...]

Verifies initialize + tools/list + server_info, then runs each CMD through
exec_command expecting a clean exit. Bearer auth is taken from the
CODING_TOOLS_MCP_AUTH_TOKEN environment variable (read by the shared client).
"""
from __future__ import annotations

import argparse
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from tests.compliance.mcp_client import MCPClient  # noqa: E402


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("url", help="MCP endpoint, e.g. http://127.0.0.1:8765/mcp")
    parser.add_argument("--expect-permission-mode", default=None)
    parser.add_argument("commands", nargs="*", help="commands to run via exec_command")
    args = parser.parse_args(argv)

    with MCPClient(Path.cwd(), url=args.url) as client:
        names = {tool["name"] for tool in client.list_tools()}
        for required in ("server_info", "exec_command"):
            if required not in names:
                raise SystemExit(f"tools/list is missing {required}: {sorted(names)}")
        info = client.call_tool("server_info", {})["structuredContent"]
        print(f"server_info: permission_mode={info['permission_mode']} tool_count={info['tool_count']}")
        if args.expect_permission_mode and info["permission_mode"] != args.expect_permission_mode:
            raise SystemExit(f"expected permission_mode={args.expect_permission_mode}, got {info['permission_mode']}")
        for cmd in args.commands:
            result = client.call_tool(
                "exec_command",
                {"cmd": cmd, "timeout_ms": 30000, "yield_time_ms": 30000},
            )["structuredContent"]
            if result.get("status") != "exited" or result.get("exit_code") != 0:
                raise SystemExit(f"command failed: {cmd!r} -> {result}")
            print(f"ok: {cmd}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
