from __future__ import annotations

import asyncio
import os
from contextlib import asynccontextmanager
from dataclasses import dataclass
from typing import Any

import uvicorn
from fastapi import Depends, FastAPI, HTTPException, Request
from fastapi.responses import HTMLResponse, JSONResponse

from .auth import require_request_auth
from .mcp_client import MCPClientError, StdioMCPClient
from .openapi_builder import build_openapi
from .policies import MUTATING_TOOLS, PolicyError, validate_tool_call


@dataclass(slots=True)
class GatewaySettings:
    workspace: str
    public_base_url: str
    permission_mode: str
    host: str
    port: int
    auth_type: str
    api_key: str

    @classmethod
    def from_env(cls) -> "GatewaySettings":
        return cls(
            workspace=os.environ.get("ACTIONS_WORKSPACE", "/workspace"),
            public_base_url=os.environ.get("ACTIONS_PUBLIC_BASE_URL", "http://127.0.0.1:8787"),
            permission_mode=os.environ.get("ACTIONS_PERMISSION_MODE", "trusted"),
            host=os.environ.get("ACTIONS_HOST", "0.0.0.0"),
            port=int(os.environ.get("ACTIONS_PORT", "8787")),
            auth_type=os.environ.get("ACTIONS_AUTH_TYPE", "api_key").strip() or "api_key",
            api_key=os.environ.get("ACTIONS_API_KEY", "").strip(),
        )


def create_app(
    settings: GatewaySettings | None = None,
    *,
    client: StdioMCPClient | None = None,
) -> FastAPI:
    resolved_settings = settings or GatewaySettings.from_env()
    resolved_client = client or StdioMCPClient(
        resolved_settings.workspace,
        permission_mode=resolved_settings.permission_mode,
    )

    @asynccontextmanager
    async def lifespan(app: FastAPI):
        if resolved_settings.auth_type == "api_key" and not resolved_settings.api_key:
            raise RuntimeError("ACTIONS_API_KEY is not configured")
        if resolved_settings.auth_type == "oauth":
            raise RuntimeError("ACTIONS_AUTH_TYPE=oauth is not supported by the Actions gateway yet")
        app.state.settings = resolved_settings
        app.state.client = resolved_client
        app.state.write_lock = asyncio.Lock()
        app.state.generated_openapi = {}

        await resolved_client.start()
        tools = await resolved_client.list_tools()
        app.state.generated_openapi = build_openapi(
            tools,
            public_base_url=resolved_settings.public_base_url,
            auth_type=resolved_settings.auth_type,
        )
        try:
            yield
        finally:
            await resolved_client.close()

    app = FastAPI(
        title="Coding Tools Actions Gateway",
        docs_url=None,
        redoc_url=None,
        openapi_url=None,
        lifespan=lifespan,
    )

    @app.get("/health")
    async def health() -> dict[str, Any]:
        return {
            "ok": True,
            "workspace": app.state.settings.workspace,
            "tools_loaded": len(app.state.generated_openapi.get("paths", {})),
        }

    @app.get("/openapi.json")
    async def openapi_json() -> JSONResponse:
        return JSONResponse(app.state.generated_openapi)

    @app.get("/privacy", response_class=HTMLResponse)
    async def privacy() -> str:
        return """
        <!doctype html>
        <html lang="zh-CN">
          <head>
            <meta charset="utf-8">
            <title>Coding Tools Actions Privacy</title>
          </head>
          <body>
            <h1>隐私政策</h1>
            <p>本服务仅供仓库所有者本人使用。</p>
            <p>请求内容只用于执行用户主动发起的代码操作。</p>
            <p>服务不会出售或共享请求数据。</p>
            <p>API 密钥、GitHub 令牌和环境变量不会返回给模型。</p>
          </body>
        </html>
        """

    @app.post("/actions/{tool_name}", dependencies=[Depends(require_request_auth)])
    async def execute_action(tool_name: str, request: Request) -> JSONResponse:
        body_bytes = await request.body()
        if not body_bytes:
            body: dict[str, Any] = {}
        else:
            try:
                parsed = await request.json()
            except Exception as exc:  # noqa: BLE001
                raise HTTPException(status_code=400, detail="Request body must be valid JSON") from exc
            if parsed is None:
                body = {}
            elif isinstance(parsed, dict):
                body = parsed
            else:
                raise HTTPException(status_code=400, detail="Request body must be a JSON object")

        try:
            validate_tool_call(tool_name, body)
        except PolicyError as exc:
            raise HTTPException(status_code=400, detail=str(exc)) from exc

        try:
            if tool_name in MUTATING_TOOLS:
                async with request.app.state.write_lock:
                    result = await request.app.state.client.call_tool(tool_name, body)
            else:
                result = await request.app.state.client.call_tool(tool_name, body)
        except MCPClientError as exc:
            raise HTTPException(status_code=502, detail=str(exc)) from exc

        is_error = bool(result.get("isError"))
        payload = {
            "ok": not is_error,
            "tool": tool_name,
            "structured_content": result.get("structuredContent"),
            "content": result.get("content"),
            "is_error": is_error,
        }
        return JSONResponse(payload, status_code=422 if is_error else 200)

    return app


app = create_app()


def main() -> None:
    settings = GatewaySettings.from_env()
    uvicorn.run(
        create_app(settings),
        host=settings.host,
        port=settings.port,
        reload=False,
        access_log=True,
    )


if __name__ == "__main__":
    main()
