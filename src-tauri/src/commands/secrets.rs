use tauri::State;

use crate::app_state::AppState;
use crate::error::{AppError, AppResult};
use crate::secret::SecretStore;

const ALLOWED_KEYS: &[&str] = &[
    "oauth_client_secret",
    "oauth_password",
    "oauth_token_secret",
    "bearer_token",
    "cloudflare_token",
    "actions_cloudflare_token",
    "actions_api_key",
    "actions_oauth_client_secret",
    "actions_oauth_password",
    "actions_oauth_token_secret",
    "frp_token",
    "actions_frp_token",
];

fn ensure_workspace_exists(state: &AppState, id: &str) -> AppResult<()> {
    state.with_workspaces(|store| {
        if store.get(id).is_some() {
            Ok(())
        } else {
            Err(AppError::Message(format!("workspace not found: {id}")))
        }
    })
}

fn validate_key(key: &str) -> AppResult<()> {
    if ALLOWED_KEYS.contains(&key) {
        Ok(())
    } else {
        Err(AppError::Message(format!("invalid secret key: {key}")))
    }
}

#[tauri::command]
pub fn get_workspace_secret(
    state: State<'_, AppState>,
    id: String,
    key: String,
) -> AppResult<Option<String>> {
    validate_key(&key)?;
    ensure_workspace_exists(&state, &id)?;
    SecretStore::get(&id, &key)
}

#[tauri::command]
pub fn set_workspace_secret(
    state: State<'_, AppState>,
    id: String,
    key: String,
    value: String,
) -> AppResult<()> {
    validate_key(&key)?;
    ensure_workspace_exists(&state, &id)?;
    SecretStore::set(&id, &key, &value)
}

#[tauri::command]
pub fn regenerate_workspace_secret(
    state: State<'_, AppState>,
    id: String,
    key: String,
) -> AppResult<String> {
    validate_key(&key)?;
    ensure_workspace_exists(&state, &id)?;
    SecretStore::regenerate(&id, &key)
}

/// Shared-secret helpers.

const SHARED_KEYS: &[&str] = &[
    "bearer_token",
    "oauth_client_secret",
    "oauth_password",
    "oauth_token_secret",
    "actions_api_key",
    "actions_oauth_client_secret",
    "actions_oauth_password",
    "actions_oauth_token_secret",
];

/// Keys whose regeneration should restart MCP services on shared-secret workspaces.
const MCP_SHARED_KEYS: &[&str] = &[
    "bearer_token",
    "oauth_client_secret",
    "oauth_password",
    "oauth_token_secret",
];

/// Keys whose regeneration should restart Actions services on shared-secret workspaces.
const ACTIONS_SHARED_KEYS: &[&str] = &[
    "actions_api_key",
    "actions_oauth_client_secret",
    "actions_oauth_password",
    "actions_oauth_token_secret",
];

#[tauri::command]
pub fn get_shared_secret(key: String) -> AppResult<Option<String>> {
    if !SHARED_KEYS.contains(&key.as_str()) {
        return Err(AppError::Message(format!("invalid shared key: {key}")));
    }
    SecretStore::get_shared(&key)
}

#[tauri::command]
pub fn set_shared_secret(key: String, value: String) -> AppResult<()> {
    if !SHARED_KEYS.contains(&key.as_str()) {
        return Err(AppError::Message(format!("invalid shared key: {key}")));
    }
    if value.is_empty() {
        return Err(AppError::Message("密钥不能为空。".into()));
    }
    let mut settings = crate::settings::AppSettings::load_or_default();
    settings.shared_secrets.insert(key.clone(), value);
    settings.save()
}

#[tauri::command]
pub fn regenerate_shared_secret(state: State<'_, AppState>, key: String) -> AppResult<String> {
    if !SHARED_KEYS.contains(&key.as_str()) {
        return Err(AppError::Message(format!("invalid shared key: {key}")));
    }
    let value = SecretStore::regenerate_shared(&key)?;

    // Restart running services on workspaces that use shared secrets.
    let workspaces = state.with_workspaces(|store| Ok(store.list().to_vec()))?;
    for ws in &workspaces {
        if MCP_SHARED_KEYS.contains(&key.as_str()) && ws.auth.use_shared_secrets {
            state.with_runtime(|rt| {
                if rt.is_running(&ws.id, crate::runtime::ServiceKind::Mcp) {
                    let _ = rt.restart_mcp(ws);
                }
                AppResult::Ok(())
            })?;
        }
        if ACTIONS_SHARED_KEYS.contains(&key.as_str()) && ws.actions.use_shared_secrets {
            state.with_runtime(|rt| {
                if rt.is_running(&ws.id, crate::runtime::ServiceKind::Actions) {
                    let _ = rt.restart_actions(ws);
                }
                AppResult::Ok(())
            })?;
        }
    }

    Ok(value)
}
