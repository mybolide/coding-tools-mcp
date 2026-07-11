from __future__ import annotations

import asyncio
from pathlib import Path
from typing import TYPE_CHECKING, Any

if TYPE_CHECKING:
    from coding_tools_mcp.server import Runtime


class MCPClientError(RuntimeError):
    pass


class StdioMCPClient:
    """直接复用 coding_tools_mcp 的 Runtime，不再额外拉起 stdio 子进程。"""

    def __init__(
        self,
        workspace: str,
        *,
        permission_mode: str = "trusted",
        timeout_seconds: float = 120.0,
        tool_profile: str = "full",
    ) -> None:
        self.workspace = workspace
        self.permission_mode = permission_mode
        self.timeout_seconds = timeout_seconds
        self.tool_profile = tool_profile
        self._runtime: Runtime | None = None
        self._lock = asyncio.Lock()

    async def start(self) -> None:
        if self._runtime is not None:
            return
        try:
            from coding_tools_mcp.server import Runtime

            self._runtime = Runtime(
                Path(self.workspace),
                permission_mode=self.permission_mode,
                tool_profile=self.tool_profile,
            )
        except Exception as exc:  # noqa: BLE001
            raise MCPClientError(str(exc)) from exc

    async def close(self) -> None:
        runtime = self._runtime
        self._runtime = None
        if runtime is None:
            return

        session_ids = set(runtime.sessions) | set(runtime.output_sessions)
        for session_id in session_ids:
            await asyncio.to_thread(runtime.cancel_session, session_id)

    async def call_tool(self, name: str, arguments: dict[str, Any]) -> dict[str, Any]:
        runtime = self._require_runtime()
        async with self._lock:
            try:
                return await asyncio.wait_for(
                    asyncio.to_thread(runtime.call_tool, name, arguments),
                    timeout=self.timeout_seconds,
                )
            except asyncio.TimeoutError as exc:
                raise MCPClientError(f"MCP request timed out: tools/call:{name}") from exc
            except Exception as exc:  # noqa: BLE001
                raise MCPClientError(str(exc)) from exc

    async def list_tools(self) -> list[dict[str, Any]]:
        runtime = self._require_runtime()
        async with self._lock:
            try:
                result = await asyncio.wait_for(
                    asyncio.to_thread(runtime.list_tools),
                    timeout=self.timeout_seconds,
                )
            except asyncio.TimeoutError as exc:
                raise MCPClientError("MCP request timed out: tools/list") from exc
            except Exception as exc:  # noqa: BLE001
                raise MCPClientError(str(exc)) from exc

        tools = result.get("tools", [])
        if not isinstance(tools, list):
            raise MCPClientError("Invalid tools/list response")
        return tools

    def _require_runtime(self) -> "Runtime":
        if self._runtime is None:
            raise MCPClientError("MCP runtime has not started")
        return self._runtime
