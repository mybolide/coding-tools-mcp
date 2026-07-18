use std::collections::HashMap;
use std::time::Instant;

use tauri::{AppHandle, Manager, State};

use std::time::Duration;

use crate::app_state::AppState;

use crate::error::{AppError, AppResult};

use crate::runtime::{
    await_listener_shutdown, port_busy_message, try_reclaim_previous_macos_app_port,
    wait_for_port_free, ServiceKind,
};

use crate::platform::platform;

use crate::tunnel::{
    append_profile_log, maybe_start_for_runtime, stop_for_runtime, sync_managed_runtime_routes,
    TunnelServiceKind,
};

use crate::workspace::resources::{validate_service_start, WorkspaceService};
use crate::workspace::RuntimeStatusDto;

fn profile_by_id(state: &AppState, id: &str) -> AppResult<crate::workspace::WorkspaceProfile> {
    state.with_workspaces(|store| {
        store
            .get(id)
            .cloned()
            .ok_or_else(|| AppError::Message(format!("workspace not found: {id}")))
    })
}

fn set_auto_start_intent(
    state: &AppState,
    id: &str,
    kind: ServiceKind,
    enabled: bool,
) -> AppResult<()> {
    state.with_workspaces(|store| {
        let Some(mut profile) = store.get(id).cloned() else {
            return Err(AppError::Message(format!("workspace not found: {id}")));
        };
        match kind {
            ServiceKind::Mcp => profile.runtime.auto_start = enabled,
            ServiceKind::Actions => profile.actions.auto_start = enabled,
        }
        store.update(profile)
    })
}

fn validate_start_resources(
    state: &AppState,
    id: &str,
    service: WorkspaceService,
) -> AppResult<()> {
    state.with_workspaces(|store| validate_service_start(store.list(), id, service))
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

async fn start_runtime_for_kind(
    state: &AppState,
    profile: &crate::workspace::WorkspaceProfile,
    kind: ServiceKind,
) -> AppResult<RuntimeStatusDto> {
    let port = match kind {
        ServiceKind::Mcp => profile.runtime.local_port,
        ServiceKind::Actions => profile.actions.local_port,
    };
    ensure_port_available(
        port,
        match kind {
            ServiceKind::Mcp => "本地 MCP",
            ServiceKind::Actions => "本地 Actions",
        },
    )
    .await?;

    state.with_runtime(|runtime| match kind {
        ServiceKind::Mcp => runtime.start_mcp(profile),
        ServiceKind::Actions => runtime.start_actions(profile),
    })?;
    sync_tunnel_routes_from_runtime(state).await?;

    match maybe_start_for_runtime(
        profile,
        match kind {
            ServiceKind::Mcp => TunnelServiceKind::Mcp,
            ServiceKind::Actions => TunnelServiceKind::Actions,
        },
    )
    .await
    {
        Ok(Some(url)) => {
            persist_tunnel_url(
                state,
                &profile.id,
                match kind {
                    ServiceKind::Mcp => TunnelServiceKind::Mcp,
                    ServiceKind::Actions => TunnelServiceKind::Actions,
                },
                &url,
            )?;
        }
        Ok(None) => {}
        Err(error) => {
            append_profile_log(
                &profile.id,
                match kind {
                    ServiceKind::Mcp => "stderr.log",
                    ServiceKind::Actions => "actions-stderr.log",
                },
                &format!("[watchdog] 隧道自动启动失败：{error}"),
            );
        }
    }

    tokio::time::sleep(Duration::from_millis(250)).await;
    state.with_runtime(|runtime| {
        match kind {
            ServiceKind::Mcp => runtime.refresh_mcp(profile),
            ServiceKind::Actions => runtime.refresh_actions(profile),
        }
        Ok(match kind {
            ServiceKind::Mcp => runtime.mcp_status(profile),
            ServiceKind::Actions => runtime.actions_status(profile),
        })
    })
}

async fn restart_runtime_for_kind(
    state: &AppState,
    profile: &crate::workspace::WorkspaceProfile,
    kind: ServiceKind,
) -> AppResult<RuntimeStatusDto> {
    let port = match kind {
        ServiceKind::Mcp => profile.runtime.local_port,
        ServiceKind::Actions => profile.actions.local_port,
    };
    stop_for_runtime(
        profile,
        match kind {
            ServiceKind::Mcp => TunnelServiceKind::Mcp,
            ServiceKind::Actions => TunnelServiceKind::Actions,
        },
    )
    .await?;
    let handle = state.with_runtime(|runtime| Ok(runtime.begin_stop(&profile.id, kind)))?;
    await_listener_shutdown(handle, port).await;
    state.with_runtime(|runtime| {
        runtime.finish_stop(&profile.id, kind);
        Ok(())
    })?;
    start_runtime_for_kind(state, profile, kind).await
}

/// Start remembered services after launch and recover a listener whose port
/// disappeared. This loop is intentionally local and authenticated; it only
/// acts on persisted user intent and never starts a service that the user
/// explicitly stopped.
pub async fn run_runtime_watchdog(app: AppHandle) {
    let mut last_attempts: HashMap<(String, ServiceKind), Instant> = HashMap::new();
    loop {
        let state = app.state::<AppState>();
        let profiles = match state.with_workspaces(|store| Ok(store.list().to_vec())) {
            Ok(profiles) => profiles,
            Err(error) => {
                eprintln!("runtime watchdog could not load workspaces: {error}");
                tokio::time::sleep(Duration::from_secs(5)).await;
                continue;
            }
        };

        for profile in profiles {
            for kind in [ServiceKind::Mcp, ServiceKind::Actions] {
                let (auto_start, auto_recover, status) = match state.with_runtime(|runtime| {
                    let (auto_start, auto_recover) = match kind {
                        ServiceKind::Mcp => {
                            (profile.runtime.auto_start, profile.runtime.auto_recover)
                        }
                        ServiceKind::Actions => {
                            (profile.actions.auto_start, profile.actions.auto_recover)
                        }
                    };
                    match kind {
                        ServiceKind::Mcp => runtime.refresh_mcp(&profile),
                        ServiceKind::Actions => runtime.refresh_actions(&profile),
                    }
                    Ok((
                        auto_start,
                        auto_recover,
                        match kind {
                            ServiceKind::Mcp => runtime.mcp_status(&profile),
                            ServiceKind::Actions => runtime.actions_status(&profile),
                        },
                    ))
                }) {
                    Ok(value) => value,
                    Err(error) => {
                        eprintln!("runtime watchdog status failed for {}: {error}", profile.id);
                        continue;
                    }
                };

                let should_start = status.state == "stopped" && auto_start;
                let should_recover = status.state == "error" && auto_recover;
                if !should_start && !should_recover {
                    continue;
                }
                let key = (profile.id.clone(), kind);
                if last_attempts
                    .get(&key)
                    .is_some_and(|last| last.elapsed() < Duration::from_secs(10))
                {
                    continue;
                }
                last_attempts.insert(key, Instant::now());

                let result = if should_recover {
                    restart_runtime_for_kind(&state, &profile, kind).await
                } else {
                    start_runtime_for_kind(&state, &profile, kind).await
                };
                if let Err(error) = result {
                    append_profile_log(
                        &profile.id,
                        match kind {
                            ServiceKind::Mcp => "stderr.log",
                            ServiceKind::Actions => "actions-stderr.log",
                        },
                        &format!("[watchdog] 服务自动恢复失败：{error}"),
                    );
                }
            }
        }
        tokio::time::sleep(Duration::from_secs(5)).await;
    }
}

#[tauri::command]

pub async fn start_runtime(state: State<'_, AppState>, id: String) -> AppResult<RuntimeStatusDto> {
    set_auto_start_intent(&state, &id, ServiceKind::Mcp, true)?;
    validate_start_resources(&state, &id, WorkspaceService::Mcp)?;
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
    set_auto_start_intent(&state, &id, ServiceKind::Mcp, false)?;
    let profile = profile_by_id(&state, &id)?;

    let port = profile.runtime.local_port;

    let handle = state.with_runtime(|runtime| Ok(runtime.begin_stop(&id, ServiceKind::Mcp)))?;

    await_listener_shutdown(handle, port).await;

    state.with_runtime(|runtime| {
        runtime.finish_stop(&id, ServiceKind::Mcp);

        Ok(runtime.mcp_status(&profile))
    })?;
    stop_for_runtime(&profile, TunnelServiceKind::Mcp).await?;
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
    set_auto_start_intent(&state, &id, ServiceKind::Actions, true)?;
    validate_start_resources(&state, &id, WorkspaceService::Actions)?;
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
    set_auto_start_intent(&state, &id, ServiceKind::Actions, false)?;
    let profile = profile_by_id(&state, &id)?;

    let port = profile.actions.local_port;

    let handle = state.with_runtime(|runtime| Ok(runtime.begin_stop(&id, ServiceKind::Actions)))?;

    await_listener_shutdown(handle, port).await;

    state.with_runtime(|runtime| {
        runtime.finish_stop(&id, ServiceKind::Actions);

        Ok(runtime.actions_status(&profile))
    })?;
    stop_for_runtime(&profile, TunnelServiceKind::Actions).await?;
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
    set_auto_start_intent(&state, &id, ServiceKind::Mcp, true)?;
    validate_start_resources(&state, &id, WorkspaceService::Mcp)?;
    let profile = profile_by_id(&state, &id)?;

    state.with_runtime(|runtime| runtime.restart_mcp(&profile))
}

#[tauri::command]

pub fn restart_actions_runtime(
    state: State<'_, AppState>,

    id: String,
) -> AppResult<RuntimeStatusDto> {
    set_auto_start_intent(&state, &id, ServiceKind::Actions, true)?;
    validate_start_resources(&state, &id, WorkspaceService::Actions)?;
    let profile = profile_by_id(&state, &id)?;

    state.with_runtime(|runtime| runtime.restart_actions(&profile))
}
