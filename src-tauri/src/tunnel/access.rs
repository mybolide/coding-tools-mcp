use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::LazyLock;

use std::collections::HashSet;
use std::path::PathBuf;

use tokio::sync::Mutex;
use tokio::time::{sleep, Duration};

use crate::data::DataStore;
use crate::error::AppResult;
use crate::platform::platform;
use crate::settings::AppSettings;
use crate::workspace::WorkspaceProfile;

use super::frp;
use super::{TunnelServiceKind, TunnelSupervisor};

static TUNNEL_SUPERVISOR: LazyLock<Mutex<TunnelSupervisor>> =
    LazyLock::new(|| Mutex::new(TunnelSupervisor::new()));
static FRP_HEALTH_LOOP_STARTED: AtomicBool = AtomicBool::new(false);
/// True only after a sustained offline window; avoids thrashing on probe flaps.
static HOST_NETWORK_MARKED_OFFLINE: AtomicBool = AtomicBool::new(false);
static HOST_NETWORK_OFFLINE_STREAK: AtomicU32 = AtomicU32::new(0);

const FRP_HEALTH_INTERVAL: Duration = Duration::from_secs(15);
const HOST_OFFLINE_STREAK_TO_MARK: u32 = 2;

pub fn supervisor() -> &'static Mutex<TunnelSupervisor> {
    &TUNNEL_SUPERVISOR
}

/// Start a background loop that restarts stuck FRP clients (process alive, proxy dead).
pub fn ensure_frp_health_loop() {
    if FRP_HEALTH_LOOP_STARTED.swap(true, Ordering::SeqCst) {
        return;
    }
    tauri::async_runtime::spawn(async {
        loop {
            sleep(FRP_HEALTH_INTERVAL).await;
            let settings = AppSettings::load_or_default();
            let online = frp::probe_host_network_available().await;
            let recovered = update_host_network_state(online);
            let mut guard = supervisor().lock().await;
            if recovered {
                let _ = guard
                    .restart_frpc_after_network_recovery(&settings)
                    .await;
            }
            let _ = guard.heal_unhealthy_frpc(&settings).await;
        }
    });
}

fn update_host_network_state(online: bool) -> bool {
    if online {
        HOST_NETWORK_OFFLINE_STREAK.store(0, Ordering::SeqCst);
        // Recovery only fires after we previously marked a sustained outage.
        HOST_NETWORK_MARKED_OFFLINE.swap(false, Ordering::SeqCst)
    } else {
        let streak = HOST_NETWORK_OFFLINE_STREAK.fetch_add(1, Ordering::SeqCst) + 1;
        if streak >= HOST_OFFLINE_STREAK_TO_MARK {
            HOST_NETWORK_MARKED_OFFLINE.store(true, Ordering::SeqCst);
        }
        false
    }
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

/// 应用退出前的最后一道清理：杀掉所有受管隧道子进程。
///
/// 退出回调是同步上下文，这里用 `try_lock` 抢一次锁；抢不到也不能阻塞退出，
/// 否则应用会卡在关闭流程里。抢不到时说明隧道正在被其它操作改动，此时
/// 兜底路径由下次启动的孤儿清理负责。
pub fn terminate_children_on_exit() {
    if let Ok(mut guard) = supervisor().try_lock() {
        guard.terminate_all_children_now();
    }
}

/// 启动时清理上次异常退出遗留的受管隧道进程（macOS 上会以应用身份存活）。
pub async fn terminate_recorded_tunnel_children_on_startup() -> usize {
    let mut terminated = 0usize;
    for image in managed_tunnel_images() {
        if let Ok(count) = platform().terminate_processes_by_image_path(&image) {
            terminated += count;
        }
    }
    terminated
}

fn managed_tunnel_images() -> Vec<PathBuf> {
    let mut images = Vec::new();
    if let Ok(dir) = platform().app_config_dir() {
        images.push(dir.join("bin").join("frpc"));
        images.push(dir.join("bin").join("cloudflared"));
    }
    images
}

pub async fn sync_managed_runtime_routes(
    active_runtime_keys: HashSet<(String, TunnelServiceKind)>,
) -> AppResult<()> {
    let settings = AppSettings::load_or_default();
    let profiles = DataStore::read_file(|data| Ok(data.profiles.clone()))?;
    let mut guard = supervisor().lock().await;
    guard.restore_active_frp_routes(&profiles, &active_runtime_keys, &settings);
    Ok(())
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

#[cfg(test)]
mod tests {
    use super::{
        update_host_network_state, HOST_NETWORK_MARKED_OFFLINE, HOST_NETWORK_OFFLINE_STREAK,
        HOST_OFFLINE_STREAK_TO_MARK,
    };
    use std::sync::atomic::Ordering;
    use std::sync::Mutex;

    /// 这两个测试共享进程级原子状态，并行执行会互相覆盖 `reset_network_state`
    /// 的结果，导致间歇性失败。用互斥锁让它们串行。
    static NETWORK_STATE_TEST_LOCK: Mutex<()> = Mutex::new(());

    fn reset_network_state() {
        HOST_NETWORK_MARKED_OFFLINE.store(false, Ordering::SeqCst);
        HOST_NETWORK_OFFLINE_STREAK.store(0, Ordering::SeqCst);
    }

    #[test]
    fn network_recovery_requires_sustained_offline() {
        let _guard = NETWORK_STATE_TEST_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        reset_network_state();
        assert!(!update_host_network_state(false));
        assert!(!HOST_NETWORK_MARKED_OFFLINE.load(Ordering::SeqCst));
        for _ in 1..HOST_OFFLINE_STREAK_TO_MARK {
            assert!(!update_host_network_state(false));
        }
        assert!(HOST_NETWORK_MARKED_OFFLINE.load(Ordering::SeqCst));
        assert!(update_host_network_state(true));
        assert!(!HOST_NETWORK_MARKED_OFFLINE.load(Ordering::SeqCst));
        // Online flaps without a sustained outage must not restart.
        assert!(!update_host_network_state(true));
    }

    #[test]
    fn single_offline_probe_does_not_mark_outage() {
        let _guard = NETWORK_STATE_TEST_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        reset_network_state();
        assert!(!update_host_network_state(false));
        assert!(!HOST_NETWORK_MARKED_OFFLINE.load(Ordering::SeqCst));
        // Brief flap back online must not count as recovery.
        assert!(!update_host_network_state(true));
    }
}
