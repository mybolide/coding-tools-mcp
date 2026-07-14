use tauri::{Manager, State};

use crate::app_state::AppState;
use crate::error::{AppError, AppResult};

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
    state.with_data(|store| store.get_workspace_secret(&id, &key))
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
    state.with_data(|store| store.set_workspace_secret(&id, &key, &value))
}

#[tauri::command]
pub fn regenerate_workspace_secret(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    id: String,
    key: String,
) -> AppResult<String> {
    validate_key(&key)?;
    ensure_workspace_exists(&state, &id)?;
    let value = state.with_data(|store| store.regenerate_workspace_secret(&id, &key))?;
    let profile = state.with_workspaces(|store| {
        store
            .get(&id)
            .cloned()
            .ok_or_else(|| AppError::Message(format!("workspace not found: {id}")))
    })?;

    schedule_running_services_restart(app, vec![profile], key, false);
    Ok(value)
}

const SHARED_KEYS: &[&str] = &[
    "oauth_client_id",
    "bearer_token",
    "oauth_client_secret",
    "oauth_password",
    "oauth_token_secret",
    "actions_api_key",
    "actions_oauth_client_secret",
    "actions_oauth_password",
    "actions_oauth_token_secret",
];

const MCP_SHARED_KEYS: &[&str] = &[
    "oauth_client_id",
    "bearer_token",
    "oauth_client_secret",
    "oauth_password",
    "oauth_token_secret",
];

const ACTIONS_SHARED_KEYS: &[&str] = &[
    "actions_api_key",
    "actions_oauth_client_secret",
    "actions_oauth_password",
    "actions_oauth_token_secret",
];

#[tauri::command]
pub fn get_shared_secret(state: State<'_, AppState>, key: String) -> AppResult<Option<String>> {
    if !SHARED_KEYS.contains(&key.as_str()) {
        return Err(AppError::Message(format!("invalid shared key: {key}")));
    }
    state.with_data(|store| Ok(store.get_shared_secret(&key)))
}

#[tauri::command]
pub fn set_shared_secret(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    key: String,
    value: String,
) -> AppResult<()> {
    if !SHARED_KEYS.contains(&key.as_str()) {
        return Err(AppError::Message(format!("invalid shared key: {key}")));
    }
    if value.is_empty() {
        return Err(AppError::Message("密钥不能为空。".into()));
    }
    let changed = state.with_data(|store| {
        if store.get_shared_secret(&key).as_deref() == Some(value.as_str()) {
            return Ok(false);
        }
        store.set_shared_secret(&key, &value)?;
        Ok(true)
    })?;
    if changed {
        let workspaces = state.with_workspaces(|store| Ok(store.list().to_vec()))?;
        schedule_running_services_restart(app, workspaces, key, true);
    }
    Ok(())
}

#[tauri::command]
pub fn regenerate_shared_secret(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    key: String,
) -> AppResult<String> {
    if !SHARED_KEYS.contains(&key.as_str()) {
        return Err(AppError::Message(format!("invalid shared key: {key}")));
    }
    let value = state.with_data(|store| store.regenerate_shared_secret(&key))?;

    let workspaces = state.with_workspaces(|store| Ok(store.list().to_vec()))?;
    schedule_running_services_restart(app, workspaces, key, true);

    Ok(value)
}

fn schedule_running_services_restart(
    app: tauri::AppHandle,
    profiles: Vec<crate::workspace::WorkspaceProfile>,
    key: String,
    shared: bool,
) {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        for profile in &profiles {
            restart_running_services(state.inner(), profile, &key, shared);
        }
    });
}

/// 仅重启当前确实在运行、且使用了这组密钥的服务。
///
/// 密钥命令是桌面端和设置页共用的入口，因此重启必须放在后端统一处理。
/// 前端不再额外调用 restart_*，避免同一次密钥变更触发两次停止/启动竞态。
fn restart_running_services(
    state: &AppState,
    profile: &crate::workspace::WorkspaceProfile,
    key: &str,
    shared: bool,
) {
    let result = state.with_runtime(|runtime| {
        if MCP_SHARED_KEYS.contains(&key)
            && profile.auth.use_shared_secrets == shared
            && runtime.is_running(&profile.id, crate::runtime::ServiceKind::Mcp)
        {
            if let Err(error) = runtime.restart_mcp(profile) {
                eprintln!(
                    "MCP restart after secret regeneration failed for {}: {error}",
                    profile.id
                );
            }
        }

        if ACTIONS_SHARED_KEYS.contains(&key)
            && profile.actions.use_shared_secrets == shared
            && runtime.is_running(&profile.id, crate::runtime::ServiceKind::Actions)
        {
            if let Err(error) = runtime.restart_actions(profile) {
                eprintln!(
                    "Actions restart after secret regeneration failed for {}: {error}",
                    profile.id
                );
            }
        }

        AppResult::Ok(())
    });

    if let Err(error) = result {
        eprintln!(
            "runtime state unavailable after secret regeneration for {}: {error}",
            profile.id
        );
    }
}
