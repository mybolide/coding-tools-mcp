from __future__ import annotations

import json
import urllib.error
import urllib.request
from dataclasses import dataclass

from .models import WorkspaceProfile


@dataclass
class HealthItem:
    label: str
    ok: bool
    detail: str


def _check_url(url: str) -> tuple[bool, str]:
    request = urllib.request.Request(url, method="GET")
    try:
        with urllib.request.urlopen(request, timeout=4) as response:
            return True, f"HTTP {response.status}"
    except urllib.error.HTTPError as exc:
        if exc.code in {200, 401, 404}:
            return True, f"HTTP {exc.code}"
        return False, f"HTTP {exc.code}"
    except Exception as exc:  # noqa: BLE001
        return False, str(exc)


def _check_json_field(url: str, field: str) -> tuple[bool, str]:
    request = urllib.request.Request(url, method="GET")
    try:
        with urllib.request.urlopen(request, timeout=4) as response:
            payload = json.loads(response.read().decode("utf-8", errors="replace"))
            value = payload.get(field)
            if isinstance(value, list):
                value = " / ".join(str(item) for item in value)
            return True, f"HTTP {response.status}; {field}={value}"
    except urllib.error.HTTPError as exc:
        return False, f"HTTP {exc.code}"
    except Exception as exc:  # noqa: BLE001
        return False, str(exc)


def run_health_checks(profile: WorkspaceProfile) -> list[HealthItem]:
    local_ok, local_detail = _check_url(profile.local_endpoint)
    oauth_ok, oauth_detail = _check_json_field(
        f"{profile.effective_public_url}/.well-known/oauth-authorization-server",
        "token_endpoint_auth_methods_supported",
    )
    public_ok, public_detail = _check_url(profile.endpoint)
    protected_ok, protected_detail = _check_json_field(
        f"{profile.effective_public_url}/.well-known/oauth-protected-resource",
        "authorization_servers",
    )
    return [
        HealthItem("本地 /mcp", local_ok, local_detail),
        HealthItem("公网 /mcp", public_ok, public_detail),
        HealthItem("OAuth 授权元数据", oauth_ok, oauth_detail),
        HealthItem("OAuth 受保护资源元数据", protected_ok, protected_detail),
    ]


def summarize_health(items: list[HealthItem]) -> str:
    summary = {"checks": [{"label": item.label, "ok": item.ok, "detail": item.detail} for item in items]}
    return json.dumps(summary, indent=2, ensure_ascii=False)
