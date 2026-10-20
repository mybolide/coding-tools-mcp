use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use tokio::process::Child;
use tokio::time::{sleep, Duration, Instant};

use crate::error::{AppError, AppResult};
use crate::platform::platform;
use crate::secret::SecretStore;
use crate::settings::AppSettings;
use crate::workspace::WorkspaceProfile;

use super::cloudflare::{self, CloudflareTunnelHandle};
use super::frp::{self, FrpServerConfig};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TunnelServiceKind {
    Mcp,
    Actions,
}

impl TunnelServiceKind {
    pub fn parse(service: &str) -> AppResult<Self> {
        match service.to_ascii_lowercase().as_str() {
            "mcp" => Ok(Self::Mcp),
            "actions" => Ok(Self::Actions),
            other => Err(AppError::Message(format!(
                "unknown tunnel service: {other}"
            ))),
        }
    }
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TunnelStatus {
    pub state: String,
    pub public_url: String,
    pub tunnel_pid: Option<u32>,
}

struct TunnelSession {
    public_url: String,
    pid: Option<u32>,
    child: Option<Child>,
}

struct FrpRoute {
    profile: WorkspaceProfile,
    kind: TunnelServiceKind,
}

struct FrpcProcess {
    child: Child,
    pid: Option<u32>,
}

#[derive(Default)]
struct FrpcHealthState {
    unhealthy_streak: u32,
    public_probe_failures: u32,
    last_restart_at: Option<Instant>,
}

enum FrpcHealthVerdict {
    Healthy,
    Unhealthy { reason: String },
}

pub struct TunnelSupervisor {
    sessions: HashMap<(String, TunnelServiceKind), TunnelSession>,
    frp_routes: HashMap<(String, TunnelServiceKind), FrpRoute>,
    frpc: HashMap<String, FrpcProcess>,
    frpc_health: HashMap<String, FrpcHealthState>,
}

impl Default for TunnelSupervisor {
    fn default() -> Self {
        Self::new()
    }
}

const FRPC_HEALTH_STREAK_TO_RESTART: u32 = 1;
const FRPC_HEALTH_RESTART_COOLDOWN: Duration = Duration::from_secs(60);
const PUBLIC_PROBE_FAILURE_THRESHOLD: u32 = 3;

#[allow(dead_code)]
impl TunnelSupervisor {
    pub fn new() -> Self {
        Self {
            sessions: HashMap::new(),
            frp_routes: HashMap::new(),
            frpc: HashMap::new(),
            frpc_health: HashMap::new(),
        }
    }

    /// Probe active FRP workspaces; restart frpc when process is alive but proxy is dead.
    pub async fn heal_unhealthy_frpc(&mut self, settings: &AppSettings) -> usize {
        let mut workspace_ids: HashSet<String> = self.frpc.keys().cloned().collect();
        workspace_ids.extend(self.frp_routes.keys().map(|(workspace_id, _)| workspace_id.clone()));
        let mut restarted = 0usize;
        let host_online = frp::probe_host_network_available().await;
        for workspace_id in workspace_ids {
            let verdict = self
                .evaluate_frpc_health(&workspace_id, settings, host_online)
                .await;
            let FrpcHealthVerdict::Unhealthy { reason } = verdict else {
                if let Some(state) = self.frpc_health.get_mut(&workspace_id) {
                    state.unhealthy_streak = 0;
                }
                continue;
            };

            let state = self.frpc_health.entry(workspace_id.clone()).or_default();
            state.unhealthy_streak = state.unhealthy_streak.saturating_add(1);
            let cooled_down = state
                .last_restart_at
                .map(|at| at.elapsed() >= FRPC_HEALTH_RESTART_COOLDOWN)
                .unwrap_or(true);
            if state.unhealthy_streak < FRPC_HEALTH_STREAK_TO_RESTART || !cooled_down {
                continue;
            }

            append_profile_log(
                &workspace_id,
                "frpc-mcp.log",
                &format!("[health] auto-restarting frpc: {reason}"),
            );
            match self.restart_workspace_frpc(&workspace_id, settings).await {
                Ok(()) => {
                    if let Some(state) = self.frpc_health.get_mut(&workspace_id) {
                        state.unhealthy_streak = 0;
                        state.public_probe_failures = 0;
                        state.last_restart_at = Some(Instant::now());
                    }
                    restarted += 1;
                }
                Err(error) => {
                    append_profile_log(
                        &workspace_id,
                        "frpc-mcp.log",
                        &format!("[health] auto-restart failed: {error}"),
                    );
                }
            }
        }
        restarted
    }

    /// After sustained offline→online, revive missing or unhealthy FRP clients.
    /// Healthy running clients are left alone to avoid proxy-already-exists thrash.
    pub async fn restart_frpc_after_network_recovery(&mut self, settings: &AppSettings) -> usize {
        let mut workspace_ids: HashSet<String> = self
            .frp_routes
            .keys()
            .map(|(workspace_id, _)| workspace_id.clone())
            .collect();
        workspace_ids.extend(self.frpc.keys().cloned());
        let mut restarted = 0usize;
        for workspace_id in workspace_ids {
            let verdict = self
                .evaluate_frpc_health(&workspace_id, settings, true)
                .await;
            let reason = match verdict {
                FrpcHealthVerdict::Unhealthy { reason } => reason,
                FrpcHealthVerdict::Healthy => continue,
            };
            append_profile_log(
                &workspace_id,
                "frpc-mcp.log",
                &format!("[health] host network restored, restarting frpc: {reason}"),
            );
            match self.restart_workspace_frpc(&workspace_id, settings).await {
                Ok(()) => {
                    if let Some(state) = self.frpc_health.get_mut(&workspace_id) {
                        state.unhealthy_streak = 0;
                        state.public_probe_failures = 0;
                        state.last_restart_at = Some(Instant::now());
                    }
                    restarted += 1;
                }
                Err(error) => {
                    append_profile_log(
                        &workspace_id,
                        "frpc-mcp.log",
                        &format!("[health] network-recovery restart failed: {error}"),
                    );
                }
            }
        }
        restarted
    }

    async fn evaluate_frpc_health(
        &mut self,
        workspace_id: &str,
        settings: &AppSettings,
        host_online: bool,
    ) -> FrpcHealthVerdict {
        let process_alive = self.frpc.get(workspace_id).is_some_and(|process| {
            process
                .pid
                .map(|pid| platform().is_process_alive(pid))
                .unwrap_or(true)
        });
        let has_routes = self
            .frp_routes
            .keys()
            .any(|(route_workspace_id, _)| route_workspace_id == workspace_id);

        if !process_alive {
            if !has_routes {
                return FrpcHealthVerdict::Healthy;
            }
            if !host_online {
                // Keep waiting; do not thrash spawn while DNS/router is down.
                return FrpcHealthVerdict::Healthy;
            }
            return FrpcHealthVerdict::Unhealthy {
                reason: "frpc process missing while tunnel routes are still active".into(),
            };
        }

        let routes: Vec<&FrpRoute> = self
            .frp_routes
            .iter()
            .filter(|((id, _), _)| id == workspace_id)
            .map(|(_, route)| route)
            .collect();
        if routes.is_empty() {
            if let Some(state) = self.frpc_health.get_mut(workspace_id) {
                state.public_probe_failures = 0;
            }
            return FrpcHealthVerdict::Healthy;
        }

        let log_kind = if routes.iter().any(|r| r.kind == TunnelServiceKind::Mcp) {
            TunnelServiceKind::Mcp
        } else {
            TunnelServiceKind::Actions
        };
        let log_path = log_dir_for_profile(workspace_id).join(frp::frpc_log_name(log_kind));
        let log_tail = frp::read_frpc_log_tail(&log_path);
        if frp::frpc_reconnect_loop_detected(&log_tail) {
            if !host_online {
                // loginFailExit=false lets frpc keep retrying; killing it now
                // is what previously orphaned workspaces overnight.
                return FrpcHealthVerdict::Healthy;
            }
            return FrpcHealthVerdict::Unhealthy {
                reason: "frpc reconnect loop detected in log".into(),
            };
        }

        let mut checked_public_route = false;
        let mut public_probe_failed = false;

        for route in routes {
            let (public_url, local_ok) = match route.kind {
                TunnelServiceKind::Mcp => (
                    route.profile.public_endpoint(),
                    frp::probe_local_mcp_ok(route.profile.runtime.local_port).await,
                ),
                TunnelServiceKind::Actions => {
                    let base = route.profile.actions_effective_public_url_with(settings);
                    let public_url = if base.is_empty() {
                        String::new()
                    } else {
                        format!("{}/openapi.json", base.trim_end_matches('/'))
                    };
                    (
                        public_url,
                        frp::probe_local_actions_ok(route.profile.actions.local_port).await,
                    )
                }
            };
            if public_url.is_empty() || !local_ok {
                continue;
            }

            checked_public_route = true;
            match frp::probe_public_mcp_endpoint(&public_url).await {
                frp::PublicMcpProbe::Healthy => {}
                frp::PublicMcpProbe::FrpNotRouted => {
                    if !host_online {
                        public_probe_failed = true;
                        continue;
                    }
                    return FrpcHealthVerdict::Unhealthy {
                        reason: format!(
                            "public endpoint returns FRP not-found page ({public_url})"
                        ),
                    };
                }
                frp::PublicMcpProbe::Unreachable | frp::PublicMcpProbe::Unexpected => {
                    public_probe_failed = true;
                }
            }
        }

        let state = self.frpc_health.entry(workspace_id.to_string()).or_default();
        if !checked_public_route {
            state.public_probe_failures = 0;
            return FrpcHealthVerdict::Healthy;
        }

        if public_probe_failed {
            if !host_online {
                state.public_probe_failures = 0;
                return FrpcHealthVerdict::Healthy;
            }
            state.public_probe_failures = state.public_probe_failures.saturating_add(1);
            if state.public_probe_failures >= PUBLIC_PROBE_FAILURE_THRESHOLD {
                return FrpcHealthVerdict::Unhealthy {
                    reason: format!(
                        "public endpoint failed {} consecutive health probes",
                        state.public_probe_failures
                    ),
                };
            }
            return FrpcHealthVerdict::Healthy;
        }

        state.public_probe_failures = 0;
        FrpcHealthVerdict::Healthy
    }

    pub fn frp_snippet(
        &self,
        profile: &WorkspaceProfile,
        kind: TunnelServiceKind,
        settings: &AppSettings,
    ) -> String {
        frp::frp_snippet(profile, kind, settings)
    }

    pub fn status(
        &self,
        profile: &WorkspaceProfile,
        kind: TunnelServiceKind,
        settings: &AppSettings,
    ) -> TunnelStatus {
        let key = (profile.id.clone(), kind);
        if self.session_is_running(&key) {
            if let Some(session) = self.sessions.get(&key) {
                return TunnelStatus {
                    state: "running".into(),
                    public_url: session.public_url.clone(),
                    tunnel_pid: session.pid,
                };
            }
        }

        TunnelStatus {
            state: "stopped".into(),
            public_url: public_url_for_profile(profile, kind, settings),
            tunnel_pid: None,
        }
    }

    pub fn public_url(
        &self,
        profile: &WorkspaceProfile,
        kind: TunnelServiceKind,
        settings: &AppSettings,
    ) -> String {
        let key = (profile.id.clone(), kind);
        if self.session_is_running(&key) {
            return self
                .sessions
                .get(&key)
                .map(|session| session.public_url.clone())
                .unwrap_or_default();
        }
        public_url_for_profile(profile, kind, settings)
    }

    pub fn route_profile(
        &self,
        workspace_id: &str,
        kind: TunnelServiceKind,
    ) -> Option<WorkspaceProfile> {
        self.frp_routes
            .get(&(workspace_id.to_string(), kind))
            .map(|route| route.profile.clone())
    }

    pub async fn start(
        &mut self,
        profile: &WorkspaceProfile,
        kind: TunnelServiceKind,
        settings: &AppSettings,
    ) -> AppResult<TunnelStatus> {
        let key = (profile.id.clone(), kind);
        let tunnel_type = tunnel_type_for(profile, kind);
        if self.session_is_running(&key) && tunnel_type != "frp" {
            return Ok(self.status(profile, kind, settings));
        }

        // 暂存旧状态，直到新线路完成校验并成功启动。这样配置填写错误、
        // 线路冲突或 frpc 启动失败时，当前可用线路不会因为一次点击而丢失。
        // 对 FRP 来说，这也是“替换当前 Workspace 代理”而不是先删除再重建。
        let mut previous_session = self.sessions.remove(&key);
        let mut previous_route = self.frp_routes.remove(&key);

        if let Err(error) = validate_tunnel_requirements(profile, kind, settings) {
            self.restore_route_state(&key, previous_route.take(), previous_session.take());
            return Err(error);
        }

        if tunnel_type == "frp" {
            let config = frp::frp_server_config(profile, kind, settings, None);
            if let Err(error) =
                self.validate_frp_route_compatibility(&profile.id, &config, settings)
            {
                self.restore_route_state(&key, previous_route.take(), previous_session.take());
                return Err(error);
            }

            self.frp_routes.insert(
                key.clone(),
                FrpRoute {
                    profile: profile.clone(),
                    kind,
                },
            );
            if let Err(error) = self.ensure_frpc_matches_routes(&profile.id, settings).await {
                self.frp_routes.remove(&key);
                self.restore_route_state(&key, previous_route.take(), previous_session.take());
                if let Err(rollback_error) =
                    self.ensure_frpc_matches_routes(&profile.id, settings).await
                {
                    return Err(AppError::Message(format!(
                        "启动新的 FRP 线路失败，且恢复原有线路失败：{error}; rollback: {rollback_error}"
                    )));
                }
                return Err(error);
            }

            let public_url = frp::frp_public_url(profile, kind, settings);
            let pid = self.frpc.get(&profile.id).and_then(|process| process.pid);
            self.sessions.insert(
                key,
                TunnelSession {
                    public_url: public_url.clone(),
                    pid,
                    child: None,
                },
            );
            return Ok(TunnelStatus {
                state: "running".into(),
                public_url,
                tunnel_pid: pid,
            });
        }

        if tunnel_type != "cloudflare" {
            self.restore_route_state(&key, previous_route.take(), previous_session.take());
            return Err(AppError::Message("当前仅支持 FRP 和 Cloudflare。".into()));
        }

        let (port, mode, token, named_url, log_name) = match cloudflare_config(profile, kind) {
            Ok(config) => config,
            Err(error) => {
                self.restore_route_state(&key, previous_route.take(), previous_session.take());
                return Err(error);
            }
        };
        let use_proxy = tunnel_use_proxy(profile, kind);
        let use_http2 = tunnel_cloudflare_http2(profile, kind);
        let log_path = log_dir_for_profile(&profile.id).join(log_name);
        let handle = cloudflare::spawn_cloudflare_tunnel(
            port,
            std::path::Path::new(&profile.path),
            &log_path,
            mode,
            &token,
            &named_url,
            use_proxy,
            use_http2,
        )
        .await
        .inspect_err(|_| {
            self.restore_route_state(&key, previous_route.take(), previous_session.take());
        })?;

        let CloudflareTunnelHandle {
            child,
            public_url,
            pid,
        } = handle;

        self.sessions.insert(
            key,
            TunnelSession {
                public_url: public_url.clone(),
                pid,
                child: Some(child),
            },
        );

        Ok(TunnelStatus {
            state: "running".into(),
            public_url,
            tunnel_pid: pid,
        })
    }

    pub async fn stop(
        &mut self,
        profile: &WorkspaceProfile,
        kind: TunnelServiceKind,
        settings: &AppSettings,
    ) -> AppResult<()> {
        self.stop_internal(&profile.id, kind, settings).await
    }

    async fn stop_internal(
        &mut self,
        workspace_id: &str,
        kind: TunnelServiceKind,
        settings: &AppSettings,
    ) -> AppResult<()> {
        let key = (workspace_id.to_string(), kind);
        if let Some(route) = self.frp_routes.remove(&key) {
            let session = self.sessions.remove(&key);
            if let Err(error) = self
                .ensure_frpc_matches_routes(workspace_id, settings)
                .await
            {
                self.frp_routes.insert(key.clone(), route);
                if let Some(session) = session {
                    self.sessions.insert(key, session);
                }
                if let Err(rollback_error) = self
                    .ensure_frpc_matches_routes(workspace_id, settings)
                    .await
                {
                    return Err(AppError::Message(format!(
                        "停止 FRP 线路失败，且恢复原有线路失败：{error}; rollback: {rollback_error}"
                    )));
                }
                return Err(error);
            }
            return Ok(());
        }

        let Some(mut session) = self.sessions.remove(&key) else {
            return Ok(());
        };

        if let Some(child) = session.child.take() {
            let _ = cloudflare::stop_child(child, session.pid).await;
        } else if let Some(pid) = session.pid {
            let _ = platform().terminate_process_tree(pid);
        }
        Ok(())
    }

    /// 应用退出时的同步兜底清理。
    ///
    /// Tauri 退出回调不是 async 上下文，且 tokio 运行时可能已经开始收尾，
    /// 因此这里只做同步信号：直接杀掉所有已记录的子进程 PID，并清空内部状态。
    /// macOS 上残留的 frpc / cloudflared 会继续以工作区目录为 cwd 存活，
    /// 触发系统文件夹授权弹窗，必须在进程退出前清理。
    pub fn terminate_all_children_now(&mut self) {
        for (_, mut frpc) in self.frpc.drain() {
            if let Some(pid) = frpc.pid {
                let _ = platform().terminate_process_tree(pid);
            }
            let _ = frpc.child.start_kill();
        }
        for (_, mut session) in self.sessions.drain() {
            if let Some(pid) = session.pid {
                let _ = platform().terminate_process_tree(pid);
            }
            if let Some(child) = session.child.as_mut() {
                let _ = child.start_kill();
            }
        }
        self.frp_routes.clear();
        self.frpc_health.clear();
    }

    pub async fn drop_workspace(&mut self, workspace_id: &str) -> AppResult<()> {
        let settings = AppSettings::load_or_default();
        let keys = [
            (workspace_id.to_string(), TunnelServiceKind::Mcp),
            (workspace_id.to_string(), TunnelServiceKind::Actions),
        ];

        // 非 FRP session 正常情况下必须持有 Child。先完成归属预检，再修改
        // FRP route；不能确认归属时保持所有线路原样，避免部分删除。
        for key in &keys {
            if !self.frp_routes.contains_key(key)
                && self
                    .sessions
                    .get(key)
                    .is_some_and(|session| session.child.is_none())
            {
                return Err(AppError::Message(format!(
                    "无法确认工作区 {} 的 {} 隧道进程归属，已取消删除。",
                    workspace_id,
                    tunnel_service_label(key.1)
                )));
            }
        }

        let mut removed_routes = Vec::new();

        for key in &keys {
            if let Some(route) = self.frp_routes.remove(key) {
                let session = self.sessions.remove(key);
                removed_routes.push((key.clone(), route, session));
            }
        }

        if !removed_routes.is_empty() {
            if let Err(error) = self
                .ensure_frpc_matches_routes(workspace_id, &settings)
                .await
            {
                for (key, route, session) in removed_routes {
                    self.frp_routes.insert(key.clone(), route);
                    if let Some(session) = session {
                        self.sessions.insert(key, session);
                    }
                }
                if let Err(rollback_error) = self
                    .ensure_frpc_matches_routes(workspace_id, &settings)
                    .await
                {
                    return Err(AppError::Message(format!(
                        "删除工作区 FRP 线路失败，且恢复原有线路失败：{error}; rollback: {rollback_error}"
                    )));
                }
                return Err(error);
            }
        }

        for key in keys {
            let Some(mut session) = self.sessions.remove(&key) else {
                continue;
            };
            let child = session.child.take().ok_or_else(|| {
                AppError::Message("隧道进程归属状态在删除期间发生变化，已停止操作。".into())
            })?;
            cloudflare::stop_child(child, session.pid).await?;
        }

        Ok(())
    }

    /// Terminate a supervised tunnel when the local runtime is not listening.
    pub async fn cleanup_orphan(
        &mut self,
        profile: &WorkspaceProfile,
        kind: TunnelServiceKind,
        runtime_listening: bool,
    ) -> AppResult<()> {
        if runtime_listening {
            return Ok(());
        }
        let settings = AppSettings::load_or_default();
        let key = (profile.id.clone(), kind);
        if self.frp_routes.contains_key(&key)
            && !self.frp_route_matches(&key, profile, kind, &settings)
        {
            // 清理任务携带的是旧 runtime/profile；当前 route 已被新的端口、
            // subdomain 或配置替换，不能按相同 workspace key 删除新线路。
            return Ok(());
        }
        self.stop_internal(&profile.id, kind, &settings).await
    }

    pub fn restore_active_frp_routes(
        &mut self,
        profiles: &[WorkspaceProfile],
        active_runtime_keys: &HashSet<(String, TunnelServiceKind)>,
        settings: &AppSettings,
    ) {
        let mut changed_workspaces = HashSet::new();
        for profile in profiles {
            for kind in [TunnelServiceKind::Mcp, TunnelServiceKind::Actions] {
                let key = (profile.id.clone(), kind);
                if tunnel_type_for(profile, kind) != "frp" || !active_runtime_keys.contains(&key) {
                    continue;
                }

                match self.frp_routes.get_mut(&key) {
                    Some(route) => {
                        route.profile = profile.clone();
                        changed_workspaces.insert(profile.id.clone());
                    }
                    None => {
                        self.frp_routes.insert(
                            key,
                            FrpRoute {
                                profile: profile.clone(),
                                kind,
                            },
                        );
                        changed_workspaces.insert(profile.id.clone());
                    }
                }
            }
        }

        changed_workspaces.extend(self.frpc.keys().cloned());
        for workspace_id in changed_workspaces {
            let pid = self.frpc.get(&workspace_id).and_then(|process| process.pid);
            self.sync_frp_sessions_for_workspace(settings, &workspace_id, pid);
        }
    }

    fn validate_frp_route_compatibility(
        &self,
        workspace_id: &str,
        config: &FrpServerConfig,
        settings: &AppSettings,
    ) -> AppResult<()> {
        if let Some(conflict) = self.frp_routes.values().find(|route| {
            let existing = frp::frp_server_config(&route.profile, route.kind, settings, None);
            existing
                .proxy
                .subdomain
                .trim()
                .eq_ignore_ascii_case(config.proxy.subdomain.trim())
        }) {
            return Err(AppError::Message(format!(
                "FRP 子域名“{}”已被工作区“{}”的 {} 服务使用，不能重复。",
                config.proxy.subdomain.trim(),
                conflict.profile.name,
                tunnel_service_label(conflict.kind)
            )));
        }

        let Some(existing) = self
            .frp_routes
            .iter()
            .find(|((route_workspace_id, _), _)| route_workspace_id == workspace_id)
            .map(|(_, route)| route)
        else {
            return Ok(());
        };
        let existing_config =
            frp::frp_server_config(&existing.profile, existing.kind, settings, None);
        let same_connection = existing_config.server_addr.trim() == config.server_addr.trim()
            && existing_config.server_port == config.server_port
            && existing_config.token == config.token;
        if !same_connection {
            return Err(AppError::Message(
                "同一工作区的 MCP 与 Actions 必须使用同一 FRP 服务器、端口和 Token。".into(),
            ));
        }
        Ok(())
    }

    async fn restart_workspace_frpc(
        &mut self,
        workspace_id: &str,
        settings: &AppSettings,
    ) -> AppResult<()> {
        // 工作区锁覆盖“停止旧进程 → 等待退出 → 启动新进程”的完整窗口，
        // 防止两个桌面实例同时管理同一工作区，同时允许不同工作区独立运行。
        let _operation_lock = frp::acquire_frpc_operation_lock(workspace_id).await?;

        if let Some(process) = self.frpc.remove(workspace_id) {
            let pid = process.pid;
            cloudflare::stop_child(process.child, pid).await?;
            if pid.is_some_and(|pid| platform().is_process_alive(pid)) {
                return Err(AppError::Message(format!(
                    "停止工作区 frpc 超时，PID {} 仍在运行。",
                    pid.unwrap_or_default()
                )));
            }
            frp::clear_managed_frpc_pid(workspace_id);
        }
        self.sync_frp_sessions_for_workspace(settings, workspace_id, None);

        // 仅回收当前工作区 PID 文件明确记录的 frpc。应用重启后即使
        // supervisor 丢失 Child，也不能按镜像路径批量终止其他工作区实例。
        frp::stop_recorded_frpc_instance(workspace_id).await?;

        if !self
            .frp_routes
            .keys()
            .any(|(route_workspace_id, _)| route_workspace_id == workspace_id)
        {
            return Ok(());
        }

        let route_specs: Vec<(WorkspaceProfile, TunnelServiceKind)> = self
            .frp_routes
            .iter()
            .filter(|((route_workspace_id, _), _)| route_workspace_id == workspace_id)
            .map(|(_, route)| route)
            .map(|route| (route.profile.clone(), route.kind))
            .collect();
        let route_refs: Vec<(&WorkspaceProfile, TunnelServiceKind)> = route_specs
            .iter()
            .map(|(profile, kind)| (profile, *kind))
            .collect();
        let deadline = Instant::now() + Duration::from_secs(45);
        let handle = loop {
            match frp::spawn_frpc(workspace_id, &route_refs, settings).await {
                Ok(handle) => break handle,
                Err(error)
                    if (proxy_already_exists(&error) || transient_frpc_spawn_error(&error))
                        && Instant::now() < deadline =>
                {
                    sleep(Duration::from_secs(2)).await;
                }
                Err(error) => return Err(error),
            }
        };
        let pid = handle.pid;
        self.frpc.insert(
            workspace_id.to_string(),
            FrpcProcess {
                child: handle.child,
                pid,
            },
        );
        self.sync_frp_sessions_for_workspace(settings, workspace_id, pid);
        Ok(())
    }

    async fn ensure_frpc_matches_routes(
        &mut self,
        workspace_id: &str,
        settings: &AppSettings,
    ) -> AppResult<()> {
        let has_routes = self
            .frp_routes
            .keys()
            .any(|(route_workspace_id, _)| route_workspace_id == workspace_id);
        if !has_routes {
            self.restart_workspace_frpc(workspace_id, settings).await?;
            return Ok(());
        }

        let route_specs: Vec<(WorkspaceProfile, TunnelServiceKind)> = self
            .frp_routes
            .iter()
            .filter(|((route_workspace_id, _), _)| route_workspace_id == workspace_id)
            .map(|(_, route)| route)
            .map(|route| (route.profile.clone(), route.kind))
            .collect();
        let route_refs: Vec<(&WorkspaceProfile, TunnelServiceKind)> = route_specs
            .iter()
            .map(|(profile, kind)| (profile, *kind))
            .collect();
        let expected = frp::build_frpc_toml_for_route_refs(&route_refs, settings);

        let process_alive = self.frpc.get(workspace_id).is_some_and(|process| {
            process
                .pid
                .map(|pid| platform().is_process_alive(pid))
                .unwrap_or(true)
        });

        if process_alive && frp::managed_frpc_config_matches(workspace_id, &expected)? {
            if let FrpcHealthVerdict::Unhealthy { reason } = self
                .evaluate_frpc_health(workspace_id, settings, true)
                .await
            {
                append_profile_log(
                    workspace_id,
                    "frpc-mcp.log",
                    &format!("[health] restarting stale frpc before reuse: {reason}"),
                );
                return self.restart_workspace_frpc(workspace_id, settings).await;
            }
            let pid = self.frpc.get(workspace_id).and_then(|process| process.pid);
            self.sync_frp_sessions_for_workspace(settings, workspace_id, pid);
            return Ok(());
        }

        self.restart_workspace_frpc(workspace_id, settings).await
    }

    fn sync_frp_sessions_for_workspace(
        &mut self,
        settings: &AppSettings,
        workspace_id: &str,
        pid: Option<u32>,
    ) {
        let active_keys: HashSet<_> = self
            .frp_routes
            .keys()
            .filter(|(route_workspace_id, _)| route_workspace_id == workspace_id)
            .cloned()
            .collect();
        self.sessions.retain(|key, session| {
            key.0 != workspace_id || session.child.is_some() || active_keys.contains(key)
        });

        for (key, route) in self
            .frp_routes
            .iter()
            .filter(|((route_workspace_id, _), _)| route_workspace_id == workspace_id)
        {
            let public_url = public_url_for_profile(&route.profile, route.kind, settings);
            match self.sessions.get_mut(key) {
                Some(session) => {
                    session.public_url = public_url.clone();
                    session.pid = pid;
                }
                None => {
                    self.sessions.insert(
                        key.clone(),
                        TunnelSession {
                            public_url,
                            pid,
                            child: None,
                        },
                    );
                }
            }
        }
    }

    fn restore_route_state(
        &mut self,
        key: &(String, TunnelServiceKind),
        route: Option<FrpRoute>,
        session: Option<TunnelSession>,
    ) {
        if let Some(route) = route {
            self.frp_routes.insert(key.clone(), route);
        }
        if let Some(session) = session {
            self.sessions.insert(key.clone(), session);
        }
    }

    fn frp_route_matches(
        &self,
        key: &(String, TunnelServiceKind),
        profile: &WorkspaceProfile,
        kind: TunnelServiceKind,
        settings: &AppSettings,
    ) -> bool {
        let Some(route) = self.frp_routes.get(key) else {
            return false;
        };
        let existing = frp::frp_server_config(&route.profile, route.kind, settings, None);
        let requested = frp::frp_server_config(profile, kind, settings, None);
        existing == requested
            && tunnel_use_proxy(&route.profile, route.kind) == tunnel_use_proxy(profile, kind)
    }

    fn session_is_running(&self, key: &(String, TunnelServiceKind)) -> bool {
        if self.frp_routes.contains_key(key) {
            let process_alive = self.frpc.get(&key.0).is_some_and(|process| {
                process
                    .pid
                    .map(|pid| platform().is_process_alive(pid))
                    .unwrap_or(true)
            });
            return process_alive && self.sessions.contains_key(key);
        }
        self.sessions.get(key).is_some_and(|session| {
            session
                .pid
                .map(|pid| platform().is_process_alive(pid))
                .unwrap_or(false)
        })
    }
}

fn proxy_already_exists(error: &AppError) -> bool {
    let message = error.to_string().to_ascii_lowercase();
    message.contains("proxy") && message.contains("already exists")
}

fn transient_frpc_spawn_error(error: &AppError) -> bool {
    let message = error.to_string().to_ascii_lowercase();
    message.contains("no such host")
        || message.contains("i/o timeout")
        || message.contains("i/o deadline")
        || message.contains("temporarily unavailable")
        || message.contains("connection refused")
        || message.contains("network is unreachable")
        || message.contains("try to connect to server")
        || message.contains("connect to server error")
}

fn tunnel_type_for(profile: &WorkspaceProfile, kind: TunnelServiceKind) -> &str {
    match kind {
        TunnelServiceKind::Mcp => profile.tunnel.tunnel_type.as_str(),
        TunnelServiceKind::Actions => profile.actions.tunnel_type.as_str(),
    }
}

fn tunnel_use_proxy(profile: &WorkspaceProfile, kind: TunnelServiceKind) -> bool {
    match kind {
        TunnelServiceKind::Mcp => profile.tunnel.use_proxy,
        TunnelServiceKind::Actions => profile.actions.use_proxy,
    }
}

fn tunnel_cloudflare_http2(profile: &WorkspaceProfile, kind: TunnelServiceKind) -> bool {
    match kind {
        TunnelServiceKind::Mcp => profile.tunnel.cloudflare_http2,
        TunnelServiceKind::Actions => profile.actions.cloudflare_http2,
    }
}

fn tunnel_service_label(kind: TunnelServiceKind) -> &'static str {
    match kind {
        TunnelServiceKind::Mcp => "MCP",
        TunnelServiceKind::Actions => "Actions",
    }
}

fn public_url_for_profile(
    profile: &WorkspaceProfile,
    kind: TunnelServiceKind,
    settings: &AppSettings,
) -> String {
    match kind {
        TunnelServiceKind::Mcp => profile.effective_public_url_with(settings),
        TunnelServiceKind::Actions => profile.actions_effective_public_url_with(settings),
    }
}

fn validate_tunnel_requirements(
    profile: &WorkspaceProfile,
    kind: TunnelServiceKind,
    settings: &AppSettings,
) -> AppResult<()> {
    let tunnel_type = tunnel_type_for(profile, kind);
    if tunnel_type == "frp" {
        let (profile_id, server, subdomain, port) = match kind {
            TunnelServiceKind::Mcp => (
                profile.tunnel.frp_profile_id.as_str(),
                profile.tunnel.frp_server.as_str(),
                profile.tunnel.frp_subdomain.as_str(),
                profile.tunnel.frp_server_port,
            ),
            TunnelServiceKind::Actions => (
                profile.actions.frp_profile_id.as_str(),
                profile.actions.frp_server.as_str(),
                profile.actions.frp_subdomain.as_str(),
                profile.actions.frp_server_port,
            ),
        };
        let server = resolve_frp_server(profile_id, server, settings);
        if server.trim().is_empty() {
            return Err(AppError::Message(
                "FRP 模式需要选择全局配置或填写服务器域名。".into(),
            ));
        }
        if subdomain.trim().is_empty() {
            return Err(AppError::Message("FRP 模式需要填写子域名。".into()));
        }
        if port == 0 && settings.find_frp_profile(profile_id).is_none() {
            return Err(AppError::Message("FRP 服务器端口无效。".into()));
        }
        return Ok(());
    }
    if tunnel_type != "cloudflare" {
        return Err(AppError::Message("当前仅支持 FRP 和 Cloudflare。".into()));
    }

    cloudflare::resolve_cloudflared()?;

    let (mode, token, named_url) = match kind {
        TunnelServiceKind::Mcp => (
            profile.tunnel.cloudflare_mode.as_str(),
            SecretStore::get(&profile.id, "cloudflare_token")?.unwrap_or_default(),
            profile.tunnel.public_url.clone(),
        ),
        TunnelServiceKind::Actions => (
            profile.actions.cloudflare_mode.as_str(),
            if profile.actions.cloudflare_token.trim().is_empty() {
                SecretStore::get(&profile.id, "actions_cloudflare_token")?.unwrap_or_default()
            } else {
                profile.actions.cloudflare_token.clone()
            },
            profile.actions.public_url.clone(),
        ),
    };

    if mode == "named" {
        if token.trim().is_empty() {
            return Err(AppError::Message(
                "Cloudflare 命名隧道模式需要填写 Tunnel Token。".into(),
            ));
        }
        if named_url.trim().is_empty() {
            return Err(AppError::Message(
                "Cloudflare 命名隧道模式需要填写固定公网地址。".into(),
            ));
        }
    }

    Ok(())
}

fn resolve_frp_server(profile_id: &str, inline_server: &str, settings: &AppSettings) -> String {
    if let Some(profile) = settings.find_frp_profile(profile_id) {
        return profile.server.clone();
    }
    inline_server.to_string()
}

fn cloudflare_config(
    profile: &WorkspaceProfile,
    kind: TunnelServiceKind,
) -> AppResult<(u16, &str, String, String, &'static str)> {
    match kind {
        TunnelServiceKind::Mcp => {
            let token = SecretStore::get(&profile.id, "cloudflare_token")?.unwrap_or_default();
            Ok((
                profile.runtime.local_port,
                profile.tunnel.cloudflare_mode.as_str(),
                token,
                profile.tunnel.public_url.clone(),
                "cloudflared.log",
            ))
        }
        TunnelServiceKind::Actions => {
            let token = if profile.actions.cloudflare_token.trim().is_empty() {
                SecretStore::get(&profile.id, "actions_cloudflare_token")?.unwrap_or_default()
            } else {
                profile.actions.cloudflare_token.clone()
            };
            Ok((
                profile.actions.local_port,
                profile.actions.cloudflare_mode.as_str(),
                token,
                profile.actions.public_url.clone(),
                "actions-cloudflared.log",
            ))
        }
    }
}

pub fn log_dir_for_profile(profile_id: &str) -> PathBuf {
    platform()
        .app_config_dir()
        .map(|home| home.join("logs").join(profile_id))
        .unwrap_or_else(|_| PathBuf::from("logs").join(profile_id))
}

pub fn append_profile_log(profile_id: &str, file_name: &str, line: &str) {
    use std::io::Write;

    let log_dir = log_dir_for_profile(profile_id);
    if std::fs::create_dir_all(&log_dir).is_err() {
        return;
    }
    let path = log_dir.join(file_name);
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    {
        let _ = writeln!(file, "{line}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frp_profile(name: &str, subdomain: &str) -> WorkspaceProfile {
        let mut profile = WorkspaceProfile::new(format!("C:/workspace/{name}"), Some(name.into()));
        profile.tunnel.tunnel_type = "frp".into();
        profile.tunnel.frp_server = "frp.example.com".into();
        profile.tunnel.frp_server_port = 7000;
        profile.tunnel.frp_subdomain = subdomain.into();
        profile
    }

    #[test]
    fn active_routes_reject_duplicate_subdomains_case_insensitively() {
        let settings = AppSettings::default();
        let first = frp_profile("first", "shared");
        let second = frp_profile("second", "SHARED");
        let mut supervisor = TunnelSupervisor::new();
        supervisor.frp_routes.insert(
            (first.id.clone(), TunnelServiceKind::Mcp),
            FrpRoute {
                profile: first,
                kind: TunnelServiceKind::Mcp,
            },
        );

        let config = frp::frp_server_config(&second, TunnelServiceKind::Mcp, &settings, None);
        let error = supervisor
            .validate_frp_route_compatibility(&second.id, &config, &settings)
            .unwrap_err();
        assert!(error.to_string().contains("不能重复"));
    }

    #[test]
    fn active_routes_allow_distinct_subdomains() {
        let settings = AppSettings::default();
        let first = frp_profile("first", "first");
        let second = frp_profile("second", "second");
        let mut supervisor = TunnelSupervisor::new();
        supervisor.frp_routes.insert(
            (first.id.clone(), TunnelServiceKind::Mcp),
            FrpRoute {
                profile: first,
                kind: TunnelServiceKind::Mcp,
            },
        );

        let config = frp::frp_server_config(&second, TunnelServiceKind::Mcp, &settings, None);
        assert!(supervisor
            .validate_frp_route_compatibility(&second.id, &config, &settings)
            .is_ok());
    }

    #[test]
    fn active_routes_allow_mixed_proxy_preferences() {
        let settings = AppSettings::default();
        let mut direct = frp_profile("direct", "direct");
        direct.tunnel.use_proxy = false;
        let mut proxied = frp_profile("proxied", "proxied");
        proxied.tunnel.use_proxy = true;
        let mut supervisor = TunnelSupervisor::new();
        supervisor.frp_routes.insert(
            (direct.id.clone(), TunnelServiceKind::Mcp),
            FrpRoute {
                profile: direct,
                kind: TunnelServiceKind::Mcp,
            },
        );

        let config = frp::frp_server_config(&proxied, TunnelServiceKind::Mcp, &settings, None);
        assert!(supervisor
            .validate_frp_route_compatibility(&proxied.id, &config, &settings)
            .is_ok());
    }

    #[test]
    fn different_workspaces_may_use_different_frp_servers() {
        let settings = AppSettings::default();
        let first = frp_profile("first", "first");
        let mut second = frp_profile("second", "second");
        second.tunnel.frp_server = "another-frp.example.com".into();
        let mut supervisor = TunnelSupervisor::new();
        supervisor.frp_routes.insert(
            (first.id.clone(), TunnelServiceKind::Mcp),
            FrpRoute {
                profile: first,
                kind: TunnelServiceKind::Mcp,
            },
        );

        let config = frp::frp_server_config(&second, TunnelServiceKind::Mcp, &settings, None);
        assert!(supervisor
            .validate_frp_route_compatibility(&second.id, &config, &settings)
            .is_ok());
    }

    #[test]
    fn stale_profile_does_not_match_a_replaced_route() {
        let settings = AppSettings::default();
        let current = frp_profile("demo", "aa");
        let mut stale = current.clone();
        stale.tunnel.frp_subdomain = "a".into();
        let key = (current.id.clone(), TunnelServiceKind::Mcp);
        let mut supervisor = TunnelSupervisor::new();
        supervisor.frp_routes.insert(
            key.clone(),
            FrpRoute {
                profile: current,
                kind: TunnelServiceKind::Mcp,
            },
        );

        assert!(!supervisor.frp_route_matches(&key, &stale, TunnelServiceKind::Mcp, &settings));
    }

    #[test]
    fn restore_active_frp_routes_rehydrates_all_listening_workspaces() {
        let settings = AppSettings::default();
        let first = frp_profile("first", "gp");
        let second = frp_profile("second", "lb");
        let active_runtime_keys = HashSet::from([
            (first.id.clone(), TunnelServiceKind::Mcp),
            (second.id.clone(), TunnelServiceKind::Mcp),
        ]);

        let mut supervisor = TunnelSupervisor::new();
        supervisor.restore_active_frp_routes(
            &[first.clone(), second.clone()],
            &active_runtime_keys,
            &settings,
        );

        assert_eq!(supervisor.frp_routes.len(), 2);
        assert!(supervisor
            .frp_routes
            .contains_key(&(first.id.clone(), TunnelServiceKind::Mcp)));
        assert!(supervisor
            .frp_routes
            .contains_key(&(second.id.clone(), TunnelServiceKind::Mcp)));
        assert_eq!(supervisor.sessions.len(), 2);
        assert_eq!(
            supervisor
                .sessions
                .get(&(second.id.clone(), TunnelServiceKind::Mcp))
                .map(|session| session.public_url.as_str()),
            Some("https://lb.frp.example.com")
        );
    }

    #[test]
    fn sync_frp_sessions_removes_stale_frp_entries_and_updates_urls() {
        let settings = AppSettings::default();
        let current = frp_profile("demo", "new-subdomain");
        let current_key = (current.id.clone(), TunnelServiceKind::Mcp);
        let stale_key = (current_key.0.clone(), TunnelServiceKind::Actions);
        let mut supervisor = TunnelSupervisor::new();
        supervisor.frp_routes.insert(
            current_key.clone(),
            FrpRoute {
                profile: current,
                kind: TunnelServiceKind::Mcp,
            },
        );
        supervisor.sessions.insert(
            stale_key,
            TunnelSession {
                public_url: "https://old.frp.example.com".into(),
                pid: Some(1),
                child: None,
            },
        );

        supervisor.sync_frp_sessions_for_workspace(&settings, &current_key.0, Some(42));

        assert_eq!(supervisor.sessions.len(), 1);
        assert_eq!(
            supervisor
                .sessions
                .get(&current_key)
                .map(|session| (session.public_url.as_str(), session.pid)),
            Some(("https://new-subdomain.frp.example.com", Some(42)))
        );
    }

    #[test]
    fn syncing_one_workspace_does_not_change_another_workspace_pid() {
        let settings = AppSettings::default();
        let first = frp_profile("first", "first");
        let second = frp_profile("second", "second");
        let first_key = (first.id.clone(), TunnelServiceKind::Mcp);
        let second_key = (second.id.clone(), TunnelServiceKind::Mcp);
        let mut supervisor = TunnelSupervisor::new();
        supervisor.frp_routes.insert(
            first_key.clone(),
            FrpRoute {
                profile: first,
                kind: TunnelServiceKind::Mcp,
            },
        );
        supervisor.frp_routes.insert(
            second_key.clone(),
            FrpRoute {
                profile: second,
                kind: TunnelServiceKind::Mcp,
            },
        );
        supervisor.sync_frp_sessions_for_workspace(&settings, &second_key.0, Some(99));

        supervisor.sync_frp_sessions_for_workspace(&settings, &first_key.0, Some(42));

        assert_eq!(
            supervisor.sessions.get(&first_key).and_then(|s| s.pid),
            Some(42)
        );
        assert_eq!(
            supervisor.sessions.get(&second_key).and_then(|s| s.pid),
            Some(99)
        );
    }
}
