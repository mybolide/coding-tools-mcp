use tauri::State;

use crate::app_state::AppState;
use crate::error::{AppError, AppResult};
use crate::platform::platform;
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
    let settings = state.with_settings(|store| Ok(store.settings()))?;

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
    let settings = state.with_settings(|store| Ok(store.settings()))?;

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
    let settings = state.with_settings(|store| Ok(store.settings()))?;
    let mut guard = supervisor().lock().await;
    guard.stop(&profile, kind).await?;
    Ok(guard.status(&profile, kind, &settings))
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TunnelTestResult {
    pub success: bool,
    pub public_url: String,
    pub kept_running: bool,
    pub message: String,
}

fn local_service_listening(profile: &crate::workspace::WorkspaceProfile, kind: TunnelServiceKind) -> AppResult<bool> {
    let port = match kind {
        TunnelServiceKind::Mcp => profile.runtime.local_port,
        TunnelServiceKind::Actions => profile.actions.local_port,
    };
    Ok(platform()
        .find_pid_listening_on_port(port)?
        .is_some())
}

/// Probe tunnel connectivity without leaving it running unless the local service is already up.
#[tauri::command]
pub async fn test_tunnel(
    state: State<'_, AppState>,
    id: String,
    service: String,
) -> AppResult<TunnelTestResult> {
    let profile = profile_by_id(&state, &id)?;
    let kind = TunnelServiceKind::parse(&service)?;
    let settings = state.with_settings(|store| Ok(store.settings()))?;
    let runtime_running = local_service_listening(&profile, kind)?;

    let was_tunnel_running = {
        let guard = supervisor().lock().await;
        guard.status(&profile, kind, &settings).state == "running"
    };

    let status = {
        let mut guard = supervisor().lock().await;
        if was_tunnel_running {
            guard.stop(&profile, kind).await?;
        }
        guard.start(&profile, kind, &settings).await?
    };

    let public_url = status.public_url.clone();
    let keep_tunnel = runtime_running;

    if keep_tunnel {
        persist_public_url(&state, &id, kind, &public_url)?;
        return Ok(TunnelTestResult {
            success: !public_url.is_empty() || status.state == "running",
            public_url,
            kept_running: true,
            message: if runtime_running {
                "隧道测试成功，已保持连接（服务运行中）。".into()
            } else {
                "隧道测试成功，已恢复连接。".into()
            },
        });
    }

    {
        let mut guard = supervisor().lock().await;
        guard.stop(&profile, kind).await?;
    }

    let success = !public_url.is_empty();
    let message = if public_url.is_empty() {
        "隧道进程已退出，未获取到公网地址。".into()
    } else {
        "隧道配置验证通过。本地服务未运行，测试连接已自动断开。".into()
    };

    Ok(TunnelTestResult {
        success,
        public_url,
        kept_running: false,
        message,
    })
}
