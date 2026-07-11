from __future__ import annotations

from coding_tools_actions.openapi_builder import build_openapi


def test_openapi_contains_allowed_tools_and_bearer_auth() -> None:
    schema = build_openapi(
        [
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
            {
                "name": "kill_session",
                "description": "Kill a session",
                "inputSchema": {"type": "object"},
            },
        ],
        public_base_url="https://actions.example.com",
    )

    assert "/actions/read_file" in schema["paths"]
    assert "/actions/apply_patch" in schema["paths"]
    assert "/actions/kill_session" not in schema["paths"]
    assert schema["components"]["securitySchemes"]["bearerAuth"]["scheme"] == "bearer"
    assert "ToolExecutionResponse" in schema["components"]["schemas"]
    assert (
        schema["paths"]["/actions/read_file"]["post"]["responses"]["200"]["content"]["application/json"]["schema"]["$ref"]
        == "#/components/schemas/ToolExecutionResponse"
    )
    assert schema["paths"]["/actions/apply_patch"]["post"]["x-openai-isConsequential"] is True


def test_openapi_without_auth_has_no_security_scheme() -> None:
    schema = build_openapi(
        [
            {
                "name": "read_file",
                "description": "Read a file",
                "inputSchema": {"type": "object"},
            }
        ],
        public_base_url="https://actions.example.com",
        auth_type="none",
    )

    assert "components" in schema
    assert "schemas" in schema["components"]
    assert "securitySchemes" not in schema["components"]
    assert "security" not in schema["paths"]["/actions/read_file"]["post"]
