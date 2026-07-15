use std::sync::LazyLock;

use tokio::sync::Mutex;

use crate::error::AppResult;
use crate::platform::platform;
use crate::settings::AppSettings;
use crate::workspace::WorkspaceProfile;

use super::frp;
use super::{TunnelServiceKind, TunnelSupervisor};

static TUNNEL_SUPERVISOR: LazyLock<Mutex<TunnelSupervisor>> =
    LazyLock::new(|| Mutex::new(TunnelSupervisor::new()));

pub fn supervisor() -> &'static Mutex<TunnelSupervisor> {
    &TUNNEL_SUPERVISOR
}

/// 应用启动时回收上次异常退出留下的、由本应用管理的 frpc。
pub async fn cleanup_managed_frpc_instances() -> AppResult<usize> {
    let _operation_lock = frp::acquire_frpc_operation_lock().await?;
    frp::stop_running_frpc_instances().await
}

fn tunnel_type_for(profile: &WorkspaceProfile, kind: TunnelServiceKind) -> &str {
    match kind {
        TunnelServiceKind::Mcp => profile.tunnel.tunnel_type.as_str(),
        TunnelServiceKind::Actions => profile.actions.tunnel_type.as_str(),
    }
}

pub async fn maybe_start_for_runtime(
    profile: &WorkspaceProfile,
    kind: TunnelServiceKind,
) -> AppResult<Option<String>> {
    let tunnel_type = tunnel_type_for(profile, kind);
    if tunnel_type.is_empty() || tunnel_type == "none" {
        return Ok(None);
    }
    let settings = AppSettings::load_or_default();
    let mut guard = supervisor().lock().await;
    let status = guard.start(profile, kind, &settings).await?;
    Ok(Some(status.public_url))
}

pub async fn stop_for_runtime(
    profile: &WorkspaceProfile,
    kind: TunnelServiceKind,
) -> AppResult<()> {
    let settings = AppSettings::load_or_default();
    let mut guard = supervisor().lock().await;
    guard.stop(profile, kind, &settings).await
}

pub async fn drop_workspace(workspace_id: &str) -> AppResult<()> {
    let mut guard = supervisor().lock().await;
    guard.drop_workspace(workspace_id).await
}

pub async fn cleanup_orphan_for_runtime(
    profile: &WorkspaceProfile,
    kind: TunnelServiceKind,
    runtime_listening: bool,
) -> AppResult<()> {
    let port = match kind {
        TunnelServiceKind::Mcp => profile.runtime.local_port,
        TunnelServiceKind::Actions => profile.actions.local_port,
    };
    if runtime_listening || platform().find_pid_listening_on_port(port)?.is_some() {
        return Ok(());
    }
    let mut guard = supervisor().lock().await;
    // 等待 supervisor 锁期间 runtime 可能已经恢复，再确认一次才允许删除 route。
    if platform().find_pid_listening_on_port(port)?.is_some() {
        return Ok(());
    }
    guard.cleanup_orphan(profile, kind, false).await
}
