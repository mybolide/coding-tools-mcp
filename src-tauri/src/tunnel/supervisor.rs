use std::collections::HashMap;
use std::path::PathBuf;

use tokio::process::Child;

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

pub struct TunnelSupervisor {
    sessions: HashMap<(String, TunnelServiceKind), TunnelSession>,
    frp_routes: HashMap<(String, TunnelServiceKind), FrpRoute>,
    frpc: Option<FrpcProcess>,
}

impl Default for TunnelSupervisor {
    fn default() -> Self {
        Self::new()
    }
}

#[allow(dead_code)]
impl TunnelSupervisor {
    pub fn new() -> Self {
        Self {
            sessions: HashMap::new(),
            frp_routes: HashMap::new(),
            frpc: None,
        }
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
            if let Err(error) = self.validate_frp_route_compatibility(&config, settings) {
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
            if let Err(error) = self.restart_frpc(settings).await {
                self.frp_routes.remove(&key);
                self.restore_route_state(&key, previous_route.take(), previous_session.take());
                if let Err(rollback_error) = self.restart_frpc(settings).await {
                    return Err(AppError::Message(format!(
                        "启动新的 FRP 线路失败，且恢复原有线路失败：{error}; rollback: {rollback_error}"
                    )));
                }
                return Err(error);
            }

            let public_url = frp::frp_public_url(profile, kind, settings);
            let pid = self.frpc.as_ref().and_then(|process| process.pid);
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
        let log_path = log_dir_for_profile(&profile.id).join(log_name);
        let handle = cloudflare::spawn_cloudflare_tunnel(
            port,
            std::path::Path::new(&profile.path),
            &log_path,
            mode,
            &token,
            &named_url,
            use_proxy,
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
            if let Err(error) = self.restart_frpc(settings).await {
                self.frp_routes.insert(key.clone(), route);
                if let Some(session) = session {
                    self.sessions.insert(key, session);
                }
                if let Err(rollback_error) = self.restart_frpc(settings).await {
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
            if let Err(error) = self.restart_frpc(&settings).await {
                for (key, route, session) in removed_routes {
                    self.frp_routes.insert(key.clone(), route);
                    if let Some(session) = session {
                        self.sessions.insert(key, session);
                    }
                }
                if let Err(rollback_error) = self.restart_frpc(&settings).await {
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

    fn validate_frp_route_compatibility(
        &self,
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

        let Some(existing) = self.frp_routes.values().next() else {
            return Ok(());
        };
        let existing_config =
            frp::frp_server_config(&existing.profile, existing.kind, settings, None);
        let same_connection = existing_config.server_addr.trim() == config.server_addr.trim()
            && existing_config.server_port == config.server_port
            && existing_config.token == config.token;
        if !same_connection {
            return Err(AppError::Message(
                "当前仅运行一个 frpc 进程；所有 FRP 线路必须使用同一服务器、端口和 Token。".into(),
            ));
        }
        Ok(())
    }

    async fn restart_frpc(&mut self, settings: &AppSettings) -> AppResult<()> {
        // 这把锁必须覆盖“停止旧进程 → 等待退出 → 启动新进程”的完整窗口，
        // 否则两个桌面实例仍可能分别 stop 后同时 spawn。
        let _operation_lock = frp::acquire_frpc_operation_lock().await?;

        if let Some(process) = self.frpc.take() {
            let _ = cloudflare::stop_child(process.child, process.pid).await;
        }
        self.update_frp_pids(None);

        // 一个桌面应用只允许存在一个由我们控制的 frpc。新增 Workspace
        // 或测试连接时，先停止当前实例，再用全部线路生成新配置。
        // 这里按完整镜像路径清理是应用级兜底，处理客户端重启后 supervisor
        // 丢失 Child、但旧 frpc 仍由本应用留下的情况；不是按名称误杀其它程序。
        frp::stop_running_frpc_instances().await?;

        if self.frp_routes.is_empty() {
            return Ok(());
        }

        let route_specs: Vec<(WorkspaceProfile, TunnelServiceKind)> = self
            .frp_routes
            .values()
            .map(|route| (route.profile.clone(), route.kind))
            .collect();
        let route_refs: Vec<(&WorkspaceProfile, TunnelServiceKind)> = route_specs
            .iter()
            .map(|(profile, kind)| (profile, *kind))
            .collect();
        let handle = frp::spawn_frpc(&route_refs, settings).await?;
        let pid = handle.pid;
        self.frpc = Some(FrpcProcess {
            child: handle.child,
            pid,
        });
        self.update_frp_pids(pid);
        Ok(())
    }

    fn update_frp_pids(&mut self, pid: Option<u32>) {
        let keys: Vec<_> = self.frp_routes.keys().cloned().collect();
        for key in keys {
            if let Some(session) = self.sessions.get_mut(&key) {
                session.pid = pid;
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
            let process_alive = self.frpc.as_ref().is_some_and(|process| {
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
            .validate_frp_route_compatibility(&config, &settings)
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
            .validate_frp_route_compatibility(&config, &settings)
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
            .validate_frp_route_compatibility(&config, &settings)
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
}
