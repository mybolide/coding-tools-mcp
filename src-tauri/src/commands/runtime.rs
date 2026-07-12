use tauri::State;

use crate::app_state::AppState;
use crate::error::{AppError, AppResult};
use crate::tunnel::{maybe_start_for_runtime, stop_for_runtime, TunnelServiceKind};
use crate::workspace::RuntimeStatusDto;

fn profile_by_id(state: &AppState, id: &str) -> AppResult<crate::workspace::WorkspaceProfile> {
    state.with_workspaces(|store| {
        store
            .get(id)
            .cloned()
            .ok_or_else(|| AppError::Message(format!("workspace not found: {id}")))
    })
}

fn persist_tunnel_url(state: &AppState, id: &str, kind: TunnelServiceKind, url: &str) -> AppResult<()> {
    if url.is_empty() {
        return Ok(());
    }
    state.with_workspaces(|store| {
        let Some(mut profile) = store.get(id).cloned() else {
            return Ok(());
        };
        match kind {
            TunnelServiceKind::Mcp => profile.tunnel.public_url = url.to_string(),
            TunnelServiceKind::Actions => profile.actions.public_url = url.to_string(),
        }
        store.update(profile)?;
        Ok(())
    })
}

#[tauri::command]
pub async fn start_runtime(state: State<'_, AppState>, id: String) -> AppResult<RuntimeStatusDto> {
    let profile = profile_by_id(&state, &id)?;
    let status = state.with_runtime(|runtime| runtime.start_mcp(&profile))?;
    if let Ok(Some(url)) = maybe_start_for_runtime(&profile, TunnelServiceKind::Mcp).await {
        let _ = persist_tunnel_url(&state, &id, TunnelServiceKind::Mcp, &url);
    }
    Ok(status)
}

#[tauri::command]
pub async fn stop_runtime(state: State<'_, AppState>, id: String) -> AppResult<RuntimeStatusDto> {
    let profile = profile_by_id(&state, &id)?;
    stop_for_runtime(&profile, TunnelServiceKind::Mcp).await;
    state.with_runtime(|runtime| runtime.stop_mcp(&profile))
}

#[tauri::command]
pub fn get_runtime_status(state: State<'_, AppState>, id: String) -> AppResult<RuntimeStatusDto> {
    let profile = profile_by_id(&state, &id)?;
    state.with_runtime(|runtime| {
        runtime.refresh_mcp(&profile);
        Ok(runtime.mcp_status(&profile))
    })
}

#[tauri::command]
pub async fn start_actions_runtime(
    state: State<'_, AppState>,
    id: String,
) -> AppResult<RuntimeStatusDto> {
    let profile = profile_by_id(&state, &id)?;
    let status = state.with_runtime(|runtime| runtime.start_actions(&profile))?;
    if let Ok(Some(url)) = maybe_start_for_runtime(&profile, TunnelServiceKind::Actions).await {
        let _ = persist_tunnel_url(&state, &id, TunnelServiceKind::Actions, &url);
    }
    Ok(status)
}

#[tauri::command]
pub async fn stop_actions_runtime(
    state: State<'_, AppState>,
    id: String,
) -> AppResult<RuntimeStatusDto> {
    let profile = profile_by_id(&state, &id)?;
    stop_for_runtime(&profile, TunnelServiceKind::Actions).await;
    state.with_runtime(|runtime| runtime.stop_actions(&profile))
}

#[tauri::command]
pub fn get_actions_runtime_status(
    state: State<'_, AppState>,
    id: String,
) -> AppResult<RuntimeStatusDto> {
    let profile = profile_by_id(&state, &id)?;
    state.with_runtime(|runtime| {
        runtime.refresh_actions(&profile);
        Ok(runtime.actions_status(&profile))
    })
}

#[tauri::command]
pub fn restart_runtime(state: State<'_, AppState>, id: String) -> AppResult<RuntimeStatusDto> {
    let profile = profile_by_id(&state, &id)?;
    state.with_runtime(|runtime| runtime.restart_mcp(&profile))
}

#[tauri::command]
pub fn restart_actions_runtime(
    state: State<'_, AppState>,
    id: String,
) -> AppResult<RuntimeStatusDto> {
    let profile = profile_by_id(&state, &id)?;
    state.with_runtime(|runtime| runtime.restart_actions(&profile))
}
