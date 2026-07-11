from __future__ import annotations

import json
from pathlib import Path

from .models import WorkspaceProfile


APP_HOME = Path.home() / ".coding-tools-mcp-desktop"
PROFILES_FILE = APP_HOME / "profiles.json"
SECRETS_FILE = APP_HOME / "secrets.json"
STATE_DIR = APP_HOME / "state"


def ensure_storage() -> None:
    APP_HOME.mkdir(parents=True, exist_ok=True)
    STATE_DIR.mkdir(parents=True, exist_ok=True)


def load_profiles() -> list[WorkspaceProfile]:
    ensure_storage()
    if not PROFILES_FILE.exists():
        return []
    data = json.loads(PROFILES_FILE.read_text(encoding="utf-8"))
    profiles = [WorkspaceProfile.from_record(item) for item in data.get("profiles", [])]
    secrets = json.loads(SECRETS_FILE.read_text(encoding="utf-8")) if SECRETS_FILE.exists() else {}
    for profile in profiles:
        if profile.id in secrets:
            secret = secrets[profile.id]
            profile.tunnel.cloudflare_token = secret.get("cloudflare_token", profile.tunnel.cloudflare_token)
            profile.actions.cloudflare_token = secret.get("actions_cloudflare_token", profile.actions.cloudflare_token)
            profile.actions.api_key = secret.get("actions_api_key", profile.actions.api_key)
            profile.actions.oauth_client_secret = secret.get(
                "actions_oauth_client_secret",
                profile.actions.oauth_client_secret,
            )
            profile.auth.oauth_client_secret = secret.get("oauth_client_secret", profile.auth.oauth_client_secret)
            profile.auth.oauth_password = secret.get("oauth_password", profile.auth.oauth_password)
            profile.auth.oauth_token_secret = secret.get("oauth_token_secret", profile.auth.oauth_token_secret)
            profile.auth.bearer_token = secret.get("bearer_token", profile.auth.bearer_token)
    return profiles


def save_profiles(profiles: list[WorkspaceProfile]) -> None:
    ensure_storage()
    public_records = []
    secret_records: dict[str, dict[str, str]] = {}
    for profile in profiles:
        record = profile.to_record()
        record["tunnel"]["cloudflare_token"] = ""
        record["actions"]["cloudflare_token"] = ""
        record["actions"]["api_key"] = ""
        record["actions"]["oauth_client_secret"] = ""
        record["auth"]["oauth_client_secret"] = ""
        record["auth"]["oauth_password"] = ""
        record["auth"]["oauth_token_secret"] = ""
        record["auth"]["bearer_token"] = ""
        public_records.append(record)
        secret_records[profile.id] = {
            "cloudflare_token": profile.tunnel.cloudflare_token,
            "actions_cloudflare_token": profile.actions.cloudflare_token,
            "actions_api_key": profile.actions.api_key,
            "actions_oauth_client_secret": profile.actions.oauth_client_secret,
            "oauth_client_secret": profile.auth.oauth_client_secret,
            "oauth_password": profile.auth.oauth_password,
            "oauth_token_secret": profile.auth.oauth_token_secret,
            "bearer_token": profile.auth.bearer_token,
        }

    PROFILES_FILE.write_text(json.dumps({"profiles": public_records}, indent=2) + "\n", encoding="utf-8")
    SECRETS_FILE.write_text(json.dumps(secret_records, indent=2) + "\n", encoding="utf-8")


def log_dir_for_profile(profile_id: str) -> Path:
    ensure_storage()
    target = STATE_DIR / profile_id
    target.mkdir(parents=True, exist_ok=True)
    return target


def runtime_state_file_for_profile(profile_id: str) -> Path:
    return log_dir_for_profile(profile_id) / "runtime.json"
