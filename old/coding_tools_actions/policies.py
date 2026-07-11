from __future__ import annotations

import os
import re
import shlex
from typing import Any


ALLOWED_TOOLS = {
    "server_info",
    "check_exec_environment",
    "read_file",
    "list_dir",
    "list_files",
    "search_text",
    "apply_patch",
    "exec_command",
    "read_output",
    "git_status",
    "git_diff",
    "git_log",
    "git_show",
    "git_blame",
}

MUTATING_TOOLS = {
    "apply_patch",
    "exec_command",
}

DEFAULT_ALLOWED_COMMANDS = {
    "pytest",
    "python",
    "python3",
    "npm",
    "npx",
    "node",
    "pnpm",
    "yarn",
    "make",
    "mvn",
    "mvnw",
    "gradle",
    "gradlew",
    "cargo",
    "go",
    "ruff",
    "mypy",
    "eslint",
    "tsc",
}

FORBIDDEN_SHELL_PATTERN = re.compile(r"[;&|><`]|\$\(|\$\{|[\r\n]")


class PolicyError(ValueError):
    pass


def allowed_commands() -> set[str]:
    configured = os.environ.get("ACTIONS_ALLOWED_COMMANDS", "").strip()
    if not configured:
        return DEFAULT_ALLOWED_COMMANDS
    return {item.strip() for item in configured.split(",") if item.strip()}


def validate_tool_call(tool_name: str, arguments: dict[str, Any]) -> None:
    if tool_name not in ALLOWED_TOOLS:
        raise PolicyError(f"Tool is not exposed: {tool_name}")

    if tool_name == "exec_command":
        validate_command(arguments)
    if tool_name == "apply_patch":
        validate_patch(arguments)


def validate_command(arguments: dict[str, Any]) -> None:
    command = arguments.get("cmd")
    if not isinstance(command, str) or not command.strip():
        raise PolicyError("exec_command requires a non-empty cmd")
    if len(command) > 4_000:
        raise PolicyError("Command is too long")
    if FORBIDDEN_SHELL_PATTERN.search(command):
        raise PolicyError("Shell chaining, redirection and expansion are not allowed")

    try:
        parts = shlex.split(command)
    except ValueError as exc:
        raise PolicyError("Invalid command syntax") from exc

    if not parts:
        raise PolicyError("Empty command")

    executable = parts[0].removeprefix("./")
    if executable not in allowed_commands():
        raise PolicyError(f"Command is not allowlisted: {executable}")

    if executable in {"python", "python3"} and "-c" in parts:
        raise PolicyError("python -c is not allowed")
    if executable == "node" and "-e" in parts:
        raise PolicyError("node -e is not allowed")
    if "env" in arguments:
        raise PolicyError("Environment variables cannot be supplied by GPT")

    timeout_ms = arguments.get("timeout_ms")
    if isinstance(timeout_ms, int) and timeout_ms > 600_000:
        raise PolicyError("Command timeout exceeds 10 minutes")


def validate_patch(arguments: dict[str, Any]) -> None:
    patch = arguments.get("patch")
    if not isinstance(patch, str) or not patch.strip():
        raise PolicyError("apply_patch requires a patch")

    max_patch_bytes = int(os.environ.get("ACTIONS_MAX_PATCH_BYTES", "200000"))
    if len(patch.encode("utf-8")) > max_patch_bytes:
        raise PolicyError("Patch is too large")

