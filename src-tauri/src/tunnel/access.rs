use std::sync::LazyLock;

use tokio::sync::Mutex;

use crate::error::AppResult;
use crate::settings::AppSettings;
use crate::workspace::WorkspaceProfile;

use super::{TunnelServiceKind, TunnelSupervisor};

static TUNNEL_SUPERVISOR: LazyLock<Mutex<TunnelSupervisor>> =
    LazyLock::new(|| Mutex::new(TunnelSupervisor::new()));

pub fn supervisor() -> &'static Mutex<TunnelSupervisor> {
    &TUNNEL_SUPERVISOR
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

pub async fn stop_for_runtime(profile: &WorkspaceProfile, kind: TunnelServiceKind) {
    let mut guard = supervisor().lock().await;
    let _ = guard.stop(profile, kind).await;
}

pub async fn drop_workspace(workspace_id: &str) {
    let mut guard = supervisor().lock().await;
    guard.drop_workspace(workspace_id).await;
}

pub async fn cleanup_orphan_for_runtime(
    profile: &WorkspaceProfile,
    kind: TunnelServiceKind,
    runtime_listening: bool,
) {
    let mut guard = supervisor().lock().await;
    let _ = guard
        .cleanup_orphan(profile, kind, runtime_listening)
        .await;
}
