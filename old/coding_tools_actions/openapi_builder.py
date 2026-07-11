from __future__ import annotations

from typing import Any

from .policies import ALLOWED_TOOLS, MUTATING_TOOLS


def _content_part_schema() -> dict[str, Any]:
    return {
        "type": "object",
        "properties": {
            "type": {"type": "string"},
            "text": {"type": "string"},
            "mimeType": {"type": "string"},
            "data": {"type": "string"},
        },
        "required": ["type"],
        "additionalProperties": True,
    }


def _tool_error_schema() -> dict[str, Any]:
    return {
        "type": "object",
        "properties": {
            "code": {"type": "string"},
            "message": {"type": "string"},
            "category": {"type": "string"},
            "retryable": {"type": "boolean"},
            "details": {
                "type": "object",
                "properties": {},
                "additionalProperties": True,
            },
        },
        "additionalProperties": True,
    }


def _structured_content_schema() -> dict[str, Any]:
    return {
        "type": "object",
        "properties": {
            "ok": {"type": "boolean"},
            "error": _tool_error_schema(),
            "diagnostics": {
                "type": "object",
                "properties": {},
                "additionalProperties": True,
            },
            "permission_request": {
                "type": "object",
                "properties": {
                    "tool_name": {"type": "string"},
                    "permission": {"type": "string"},
                    "status": {"type": "string"},
                    "retryable": {"type": "boolean"},
                },
                "additionalProperties": True,
            },
        },
        "additionalProperties": True,
    }


def _tool_response_schema_ref() -> dict[str, Any]:
    return {"$ref": "#/components/schemas/ToolExecutionResponse"}


def build_openapi(
    tools: list[dict[str, Any]],
    *,
    public_base_url: str,
    auth_type: str = "api_key",
) -> dict[str, Any]:
    paths: dict[str, Any] = {}
    security = [{"bearerAuth": []}] if auth_type == "api_key" else []
    for tool in tools:
        name = tool.get("name")
        if name not in ALLOWED_TOOLS:
            continue

        input_schema = tool.get("inputSchema")
        if not isinstance(input_schema, dict):
            input_schema = {
                "type": "object",
                "additionalProperties": True,
            }

        description = str(tool.get("description") or f"Call coding tool {name}")
        operation: dict[str, Any] = {
            "operationId": f"coding_{name}",
            "summary": description[:100],
            "description": description,
            "requestBody": {
                "required": False,
                "content": {
                    "application/json": {
                        "schema": input_schema,
                    }
                },
            },
            "responses": {
                "200": {
                    "description": "Tool execution result",
                    "content": {
                        "application/json": {
                            "schema": _tool_response_schema_ref(),
                        }
                    },
                },
                "400": {"description": "Invalid request or policy rejection"},
                "401": {"description": "Invalid API key"},
                "422": {"description": "Tool execution failed"},
                "502": {"description": "MCP backend failure"},
            },
            "x-openai-isConsequential": name in MUTATING_TOOLS,
        }
        if security:
            operation["security"] = security
        paths[f"/actions/{name}"] = {"post": operation}

    document = {
        "openapi": "3.1.0",
        "info": {
            "title": "Coding Tools Actions",
            "version": "0.1.0",
            "description": "Read, modify and test a workspace through coding-tools-mcp.",
        },
        "servers": [{"url": public_base_url.rstrip("/")}],
        "paths": paths,
        "components": {
            "schemas": {
                "ContentPart": _content_part_schema(),
                "ToolError": _tool_error_schema(),
                "StructuredContent": _structured_content_schema(),
                "ToolExecutionResponse": {
                    "type": "object",
                    "properties": {
                        "ok": {"type": "boolean"},
                        "tool": {"type": "string"},
                        "structured_content": {"$ref": "#/components/schemas/StructuredContent"},
                        "content": {
                            "type": "array",
                            "items": {"$ref": "#/components/schemas/ContentPart"},
                        },
                        "is_error": {"type": "boolean"},
                    },
                    "required": ["ok", "tool", "is_error"],
                    "additionalProperties": True,
                },
            }
        },
    }
    if auth_type == "api_key":
        document["components"]["securitySchemes"] = {
            "bearerAuth": {
                "type": "http",
                "scheme": "bearer",
            }
        }
    return document
