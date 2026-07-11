from __future__ import annotations

from typing import Any

from fastapi.testclient import TestClient

from coding_tools_actions.app import GatewaySettings, create_app


class FakeClient:
    def __init__(self) -> None:
        self.calls: list[tuple[str, dict[str, Any]]] = []
        self.started = False
        self.closed = False
        self.result_by_tool: dict[str, dict[str, Any]] = {
            "read_file": {
                "structuredContent": {"ok": True, "text": "hello"},
                "content": [{"type": "text", "text": '{"ok": true, "text": "hello"}'}],
                "isError": False,
            },
            "apply_patch": {
                "structuredContent": {"ok": False, "reason": "patch failed"},
                "content": [{"type": "text", "text": '{"ok": false, "reason": "patch failed"}'}],
                "isError": True,
            },
        }

    async def start(self) -> None:
        self.started = True

    async def close(self) -> None:
        self.closed = True

    async def list_tools(self) -> list[dict[str, Any]]:
        return [
            {
                "name": "read_file",
                "description": "Read a file",
                "inputSchema": {"type": "object", "properties": {"path": {"type": "string"}}},
            },
            {
                "name": "apply_patch",
                "description": "Apply a patch",
                "inputSchema": {"type": "object", "properties": {"patch": {"type": "string"}}},
            },
        ]

    async def call_tool(self, name: str, arguments: dict[str, Any]) -> dict[str, Any]:
        self.calls.append((name, arguments))
        return self.result_by_tool[name]


def build_test_client(fake_client: FakeClient) -> TestClient:
    app = create_app(
        GatewaySettings(
            workspace="/workspace",
            public_base_url="https://actions.example.com",
            permission_mode="trusted",
            host="127.0.0.1",
            port=8787,
            auth_type="api_key",
            api_key="test-key",
        ),
        client=fake_client,
    )
    return TestClient(app)


def test_missing_api_key_returns_401() -> None:
    with build_test_client(FakeClient()) as client:
        response = client.post("/actions/read_file", json={"path": "README.md"})
    assert response.status_code == 401


def test_wrong_api_key_returns_401() -> None:
    with build_test_client(FakeClient()) as client:
        response = client.post(
            "/actions/read_file",
            headers={"Authorization": "Bearer wrong"},
            json={"path": "README.md"},
        )
    assert response.status_code == 401


def test_valid_api_key_is_accepted() -> None:
    fake_client = FakeClient()
    with build_test_client(fake_client) as client:
        response = client.post(
            "/actions/read_file",
            headers={"Authorization": "Bearer test-key"},
            json={"path": "README.md"},
        )
    assert response.status_code == 200
    assert response.json()["ok"] is True
    assert fake_client.calls == [("read_file", {"path": "README.md"})]


def test_openapi_endpoint_is_generated() -> None:
    with build_test_client(FakeClient()) as client:
        response = client.get("/openapi.json")
    assert response.status_code == 200
    assert "/actions/read_file" in response.json()["paths"]


def test_tool_error_returns_422() -> None:
    with build_test_client(FakeClient()) as client:
        response = client.post(
            "/actions/apply_patch",
            headers={"Authorization": "Bearer test-key"},
            json={"patch": "*** Begin Patch\n*** End Patch"},
        )
    assert response.status_code == 422
    assert response.json()["is_error"] is True


def test_noauth_mode_skips_header_check() -> None:
    fake_client = FakeClient()
    app = create_app(
        GatewaySettings(
            workspace="/workspace",
            public_base_url="https://actions.example.com",
            permission_mode="trusted",
            host="127.0.0.1",
            port=8787,
            auth_type="none",
            api_key="",
        ),
        client=fake_client,
    )
    with TestClient(app) as client:
        response = client.post("/actions/read_file", json={"path": "README.md"})
    assert response.status_code == 200
    assert fake_client.calls == [("read_file", {"path": "README.md"})]
