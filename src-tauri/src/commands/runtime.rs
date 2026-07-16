use tauri::State;

use std::time::Duration;

use crate::app_state::AppState;

use crate::error::{AppError, AppResult};

use crate::runtime::{
    await_listener_shutdown, port_busy_message, try_reclaim_previous_macos_app_port,
    wait_for_port_free, ServiceKind,
};

use crate::platform::platform;

use crate::tunnel::{
    maybe_start_for_runtime, stop_for_runtime, sync_managed_runtime_routes, TunnelServiceKind,
};

use crate::workspace::RuntimeStatusDto;

fn profile_by_id(state: &AppState, id: &str) -> AppResult<crate::workspace::WorkspaceProfile> {
    state.with_workspaces(|store| {
        store
            .get(id)
            .cloned()
            .ok_or_else(|| AppError::Message(format!("workspace not found: {id}")))
    })
}

fn persist_tunnel_url(
    state: &AppState,
    id: &str,
    kind: TunnelServiceKind,
    url: &str,
) -> AppResult<()> {
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

async fn sync_tunnel_routes_from_runtime(state: &AppState) -> AppResult<()> {
    let active_keys = state.with_runtime(|runtime| Ok(runtime.active_tunnel_service_keys()))?;
    sync_managed_runtime_routes(active_keys).await
}

#[allow(clippy::collapsible_if)]
async fn ensure_port_available(port: u16, service_label: &str) -> AppResult<()> {
    let Some(pid) = platform().find_pid_listening_on_port(port)? else {
        return Ok(());
    };

    if crate::runtime::is_own_process(pid) {
        if wait_for_port_free(port, Duration::from_secs(3)).await {
            return Ok(());
        }
    }

    if try_reclaim_previous_macos_app_port(port) {
        return Ok(());
    }

    if let Some(pid) = platform().find_pid_listening_on_port(port)? {
        return Err(AppError::Message(port_busy_message(
            port,
            service_label,
            pid,
        )));
    }

    Ok(())
}

#[tauri::command]

pub async fn start_runtime(state: State<'_, AppState>, id: String) -> AppResult<RuntimeStatusDto> {
    let profile = profile_by_id(&state, &id)?;

    ensure_port_available(profile.runtime.local_port, "本地 MCP").await?;

    state.with_runtime(|runtime| runtime.start_mcp(&profile))?;
    sync_tunnel_routes_from_runtime(&state).await?;

    match maybe_start_for_runtime(&profile, TunnelServiceKind::Mcp).await {
        Ok(Some(url)) => {
            persist_tunnel_url(&state, &id, TunnelServiceKind::Mcp, &url)?;
        }

        Ok(None) => {}

        Err(error) => {
            eprintln!("mcp tunnel auto-start failed for {id}: {error}");
        }
    }

    let profile = profile_by_id(&state, &id)?;

    tokio::time::sleep(Duration::from_millis(250)).await;

    state.with_runtime(|runtime| {
        runtime.refresh_mcp(&profile);

        Ok(runtime.mcp_status(&profile))
    })
}

#[tauri::command]

pub async fn stop_runtime(state: State<'_, AppState>, id: String) -> AppResult<RuntimeStatusDto> {
    let profile = profile_by_id(&state, &id)?;

    let port = profile.runtime.local_port;

    stop_for_runtime(&profile, TunnelServiceKind::Mcp).await?;

    let handle = state.with_runtime(|runtime| Ok(runtime.begin_stop(&id, ServiceKind::Mcp)))?;

    await_listener_shutdown(handle, port).await;

    state.with_runtime(|runtime| {
        runtime.finish_stop(&id, ServiceKind::Mcp);

        Ok(runtime.mcp_status(&profile))
    })?;
    sync_tunnel_routes_from_runtime(&state).await?;
    state.with_runtime(|runtime| Ok(runtime.mcp_status(&profile)))
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

    ensure_port_available(profile.actions.local_port, "本地 Actions").await?;

    state.with_runtime(|runtime| runtime.start_actions(&profile))?;
    sync_tunnel_routes_from_runtime(&state).await?;

    match maybe_start_for_runtime(&profile, TunnelServiceKind::Actions).await {
        Ok(Some(url)) => {
            persist_tunnel_url(&state, &id, TunnelServiceKind::Actions, &url)?;
        }

        Ok(None) => {}

        Err(error) => {
            eprintln!("actions tunnel auto-start failed for {id}: {error}");
        }
    }

    let profile = profile_by_id(&state, &id)?;

    tokio::time::sleep(Duration::from_millis(250)).await;

    state.with_runtime(|runtime| {
        runtime.refresh_actions(&profile);

        Ok(runtime.actions_status(&profile))
    })
}

#[tauri::command]

pub async fn stop_actions_runtime(
    state: State<'_, AppState>,

    id: String,
) -> AppResult<RuntimeStatusDto> {
    let profile = profile_by_id(&state, &id)?;

    let port = profile.actions.local_port;

    stop_for_runtime(&profile, TunnelServiceKind::Actions).await?;

    let handle = state.with_runtime(|runtime| Ok(runtime.begin_stop(&id, ServiceKind::Actions)))?;

    await_listener_shutdown(handle, port).await;

    state.with_runtime(|runtime| {
        runtime.finish_stop(&id, ServiceKind::Actions);

        Ok(runtime.actions_status(&profile))
    })?;
    sync_tunnel_routes_from_runtime(&state).await?;
    state.with_runtime(|runtime| Ok(runtime.actions_status(&profile)))
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
