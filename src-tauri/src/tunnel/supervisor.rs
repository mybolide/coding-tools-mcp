use std::collections::HashMap;
use std::path::PathBuf;

use tokio::process::Child;

use crate::error::{AppError, AppResult};
use crate::platform::platform;
use crate::secret::SecretStore;
use crate::settings::AppSettings;
use crate::workspace::WorkspaceProfile;

use super::cloudflare::{self, CloudflareTunnelHandle};
use super::frp::{self, FrpcHandle};

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
            other => Err(AppError::Message(format!("unknown tunnel service: {other}"))),
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

pub struct TunnelSupervisor {
    sessions: HashMap<(String, TunnelServiceKind), TunnelSession>,
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
        if let Some(session) = self.sessions.get(&key) {
            return TunnelStatus {
                state: "running".into(),
                public_url: session.public_url.clone(),
                tunnel_pid: session.pid,
            };
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
        self.sessions
            .get(&key)
            .map(|s| s.public_url.clone())
            .unwrap_or_else(|| public_url_for_profile(profile, kind, settings))
    }

    pub async fn start(
        &mut self,
        profile: &WorkspaceProfile,
        kind: TunnelServiceKind,
        settings: &AppSettings,
    ) -> AppResult<TunnelStatus> {
        let key = (profile.id.clone(), kind);
        if self.sessions.contains_key(&key) {
            return Ok(self.status(profile, kind, settings));
        }

        validate_tunnel_requirements(profile, kind, settings)?;

        let tunnel_type = tunnel_type_for(profile, kind);
        if tunnel_type == "frp" {
            let handle = frp::spawn_frpc(profile, kind, settings).await?;
            let FrpcHandle {
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
            return Ok(TunnelStatus {
                state: "running".into(),
                public_url,
                tunnel_pid: pid,
            });
        }

        if tunnel_type != "cloudflare" {
            return Err(AppError::Message("当前仅支持 FRP 和 Cloudflare。".into()));
        }

        let (port, mode, token, named_url, log_name) = cloudflare_config(profile, kind)?;
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
        .await?;

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
    ) -> AppResult<()> {
        self.stop_internal(&profile.id, kind).await;
        Ok(())
    }

    pub async fn stop_internal(&mut self, workspace_id: &str, kind: TunnelServiceKind) {
        let key = (workspace_id.to_string(), kind);
        let Some(mut session) = self.sessions.remove(&key) else {
            return;
        };

        if let Some(child) = session.child.take() {
            let _ = cloudflare::stop_child(child, session.pid).await;
        } else if let Some(pid) = session.pid {
            let _ = platform().terminate_process_tree(pid);
        }
    }

    pub async fn drop_workspace(&mut self, workspace_id: &str) {
        self.stop_internal(workspace_id, TunnelServiceKind::Mcp).await;
        self.stop_internal(workspace_id, TunnelServiceKind::Actions)
            .await;
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
        self.stop_internal(&profile.id, kind).await;
        Ok(())
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
