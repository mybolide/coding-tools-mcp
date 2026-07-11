use tauri::State;

use crate::app_state::AppState;
use crate::error::{AppError, AppResult};
use crate::tunnel::{frp_snippet, supervisor, TunnelServiceKind, TunnelStatus};

fn profile_by_id(state: &AppState, id: &str) -> AppResult<crate::workspace::WorkspaceProfile> {
    state.with_workspaces(|store| {
        store
            .get(id)
            .cloned()
            .ok_or_else(|| AppError::Message(format!("workspace not found: {id}")))
    })
}

fn persist_public_url(
    state: &AppState,
    id: &str,
    kind: TunnelServiceKind,
    public_url: &str,
) -> AppResult<()> {
    if public_url.is_empty() {
        return Ok(());
    }
    state.with_workspaces(|store| {
        let Some(mut profile) = store.get(id).cloned() else {
            return Ok(());
        };
        match kind {
            TunnelServiceKind::Mcp => profile.tunnel.public_url = public_url.to_string(),
            TunnelServiceKind::Actions => profile.actions.public_url = public_url.to_string(),
        }
        store.update(profile)?;
        Ok(())
    })
}

#[tauri::command]
pub fn get_frp_snippet(state: State<'_, AppState>, id: String, service: String) -> AppResult<String> {
    let profile = profile_by_id(&state, &id)?;
    let kind = TunnelServiceKind::parse(&service)?;
    Ok(frp_snippet(&profile, kind))
}

#[tauri::command]
pub async fn restart_tunnel(
    state: State<'_, AppState>,
    id: String,
    service: String,
) -> AppResult<TunnelStatus> {
    let profile = profile_by_id(&state, &id)?;
    let kind = TunnelServiceKind::parse(&service)?;
    let settings = state.with_settings(|store| Ok(store.get().clone()))?;

    let status = {
        let mut guard = supervisor().lock().await;
        let was_running = guard.status(&profile, kind, &settings).state == "running";
        if was_running {
            guard.stop(&profile, kind).await?;
            guard.start(&profile, kind, &settings).await?
        } else {
            guard.status(&profile, kind, &settings)
        }
    };

    persist_public_url(&state, &id, kind, &status.public_url)?;
    Ok(status)
}

#[tauri::command]
pub async fn start_tunnel(
    state: State<'_, AppState>,
    id: String,
    service: String,
) -> AppResult<TunnelStatus> {
    let profile = profile_by_id(&state, &id)?;
    let kind = TunnelServiceKind::parse(&service)?;
    let settings = state.with_settings(|store| Ok(store.get().clone()))?;

    let status = {
        let mut guard = supervisor().lock().await;
        guard.start(&profile, kind, &settings).await?
    };

    persist_public_url(&state, &id, kind, &status.public_url)?;
    Ok(status)
}

#[tauri::command]
pub async fn stop_tunnel(
    state: State<'_, AppState>,
    id: String,
    service: String,
) -> AppResult<TunnelStatus> {
    let profile = profile_by_id(&state, &id)?;
    let kind = TunnelServiceKind::parse(&service)?;
    let settings = state.with_settings(|store| Ok(store.get().clone()))?;
    let mut guard = supervisor().lock().await;
    guard.stop(&profile, kind).await?;
    Ok(guard.status(&profile, kind, &settings))
}
