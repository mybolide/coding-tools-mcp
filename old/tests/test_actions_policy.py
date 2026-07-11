from __future__ import annotations

import pytest

from coding_tools_actions.policies import PolicyError, validate_tool_call


def test_unlisted_tool_is_rejected() -> None:
    with pytest.raises(PolicyError, match="not exposed"):
        validate_tool_call("kill_session", {})


def test_shell_chaining_is_rejected() -> None:
    with pytest.raises(PolicyError, match="Shell chaining"):
        validate_tool_call("exec_command", {"cmd": "pytest && echo done"})


def test_python_dash_c_is_rejected() -> None:
    with pytest.raises(PolicyError, match="python -c"):
        validate_tool_call("exec_command", {"cmd": "python -c print(1)"})


def test_command_timeout_is_limited() -> None:
    with pytest.raises(PolicyError, match="10 minutes"):
        validate_tool_call("exec_command", {"cmd": "pytest", "timeout_ms": 600_001})


def test_oversized_patch_is_rejected(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setenv("ACTIONS_MAX_PATCH_BYTES", "4")
    with pytest.raises(PolicyError, match="Patch is too large"):
        validate_tool_call("apply_patch", {"patch": "abcde"})

