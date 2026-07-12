use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

use tauri::async_runtime::JoinHandle;

use crate::actions;
use crate::error::AppResult;
use crate::mcp;
use crate::platform::platform;
use crate::secret::SecretStore;
use crate::tools::policy::PolicySettings;
use crate::tunnel::{append_profile_log, cleanup_orphan_for_runtime, TunnelServiceKind};
use crate::workspace::{RuntimeStatusDto, WorkspaceProfile};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ServiceKind {
    Mcp,
    Actions,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum RuntimePhase {
    Stopped,
    Starting,
    Running,
    Stopping,
    Error,
}

struct RuntimeEntry {
    phase: RuntimePhase,
    shutdown: Option<mcp::ShutdownSender>,
    handle: Option<JoinHandle<()>>,
    error_message: Option<String>,
}

pub struct RuntimeSupervisor {
    entries: HashMap<(String, ServiceKind), RuntimeEntry>,
}

impl Default for RuntimeSupervisor {
    fn default() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }
}

impl RuntimeSupervisor {
    pub fn mcp_status(&self, profile: &WorkspaceProfile) -> RuntimeStatusDto {
        self.status(profile, ServiceKind::Mcp)
    }

    pub fn actions_status(&self, profile: &WorkspaceProfile) -> RuntimeStatusDto {
        self.status(profile, ServiceKind::Actions)
    }

    pub fn start_mcp(&mut self, profile: &WorkspaceProfile) -> AppResult<RuntimeStatusDto> {
        self.start(profile, ServiceKind::Mcp)
    }

    pub fn stop_mcp(&mut self, profile: &WorkspaceProfile) -> AppResult<RuntimeStatusDto> {
        self.stop(profile, ServiceKind::Mcp)
    }

    pub fn start_actions(&mut self, profile: &WorkspaceProfile) -> AppResult<RuntimeStatusDto> {
        self.start(profile, ServiceKind::Actions)
    }

    pub fn stop_actions(&mut self, profile: &WorkspaceProfile) -> AppResult<RuntimeStatusDto> {
        self.stop(profile, ServiceKind::Actions)
    }

    pub fn restart_mcp(&mut self, profile: &WorkspaceProfile) -> AppResult<RuntimeStatusDto> {
        self.restart(profile, ServiceKind::Mcp)
    }

    pub fn restart_actions(&mut self, profile: &WorkspaceProfile) -> AppResult<RuntimeStatusDto> {
        self.restart(profile, ServiceKind::Actions)
    }

    /// True when the service for this workspace is currently running.
    pub fn is_running(&self, workspace_id: &str, kind: ServiceKind) -> bool {
        matches!(
            self.entries
                .get(&(workspace_id.to_string(), kind))
                .map(|entry| &entry.phase),
            Some(RuntimePhase::Running)
        )
    }

    pub fn refresh_mcp(&mut self, profile: &WorkspaceProfile) {
        self.refresh(profile, ServiceKind::Mcp);
    }

    pub fn refresh_actions(&mut self, profile: &WorkspaceProfile) {
        self.refresh(profile, ServiceKind::Actions);
    }

    pub fn drop_workspace(&mut self, workspace_id: &str) {
        self.stop_internal(workspace_id, ServiceKind::Mcp);
        self.stop_internal(workspace_id, ServiceKind::Actions);
    }

    fn status(&self, profile: &WorkspaceProfile, kind: ServiceKind) -> RuntimeStatusDto {
        let key = (profile.id.clone(), kind);
        let phase = self
            .entries
            .get(&key)
            .map(|entry| entry.phase.clone())
            .unwrap_or(RuntimePhase::Stopped);

        let (local_endpoint, public_endpoint) = endpoints(profile, kind);
        let port = port_for(profile, kind);
        let service_label = service_label(kind);

        match phase {
            RuntimePhase::Running => RuntimeStatusDto {
                state: "running".into(),
                pid: None,
                local_message: format!("{service_label}正在监听 127.0.0.1:{port}"),
                public_message: public_message_for(profile, kind),
                local_endpoint,
                public_endpoint,
            },
            RuntimePhase::Starting => RuntimeStatusDto {
                state: "starting".into(),
                pid: None,
                local_message: format!("正在启动{service_label}端口 {port}"),
                public_message: "等待服务就绪".into(),
                local_endpoint,
                public_endpoint,
            },
            RuntimePhase::Stopping => RuntimeStatusDto {
                state: "stopping".into(),
                pid: None,
                local_message: "正在停止".into(),
                public_message: "正在停止".into(),
                local_endpoint,
                public_endpoint,
            },
            RuntimePhase::Error => {
                let message = self
                    .entries
                    .get(&key)
                    .and_then(|entry| entry.error_message.clone())
                    .unwrap_or_else(|| "运行失败".into());
                RuntimeStatusDto {
                    state: "error".into(),
                    pid: None,
                    local_message: message.clone(),
                    public_message: message,
                    local_endpoint,
                    public_endpoint,
                }
            }
            RuntimePhase::Stopped => RuntimeStatusDto {
                state: "stopped".into(),
                pid: None,
                local_message: "未启动".into(),
                public_message: "未知".into(),
                local_endpoint,
                public_endpoint,
            },
        }
    }

    fn start(&mut self, profile: &WorkspaceProfile, kind: ServiceKind) -> AppResult<RuntimeStatusDto> {
        let key = (profile.id.clone(), kind);
        if matches!(
            self.entries.get(&key).map(|e| &e.phase),
            Some(RuntimePhase::Running) | Some(RuntimePhase::Starting)
        ) {
            return Ok(self.status(profile, kind));
        }

        self.entries.insert(
            key.clone(),
            RuntimeEntry {
                phase: RuntimePhase::Starting,
                shutdown: None,
                handle: None,
                error_message: None,
            },
        );

        let port = port_for(profile, kind);
        if let Some(pid) = platform().find_pid_listening_on_port(port)? {
            let image = platform()
                .process_image_path(pid)?
                .unwrap_or_else(|| format!("pid {pid}"));
            self.entries.remove(&key);
            let message = format!(
                "{}端口 {} 已被占用：{}",
                service_label(kind).trim(),
                port,
                image
            );
            append_profile_log(&profile.id, stderr_log_name(kind), &format!("[start] {message}"));
            return Err(crate::error::AppError::Message(message));
        }

        let spawn_result = match kind {
            ServiceKind::Mcp => {
                let use_shared = profile.auth.use_shared_secrets;
                let oauth_client_secret = if profile.auth.oauth_enabled() {
                    resolve_secret(&profile.id, "oauth_client_secret", use_shared)?
                } else {
                    None
                };
                let oauth_password = if profile.auth.oauth_enabled() {
                    resolve_secret(&profile.id, "oauth_password", use_shared)?
                } else {
                    None
                };
                let oauth_token_secret = if profile.auth.oauth_enabled() {
                    resolve_secret(&profile.id, "oauth_token_secret", use_shared)?
                } else {
                    None
                };
                mcp::spawn_listener(
                    port,
                    PathBuf::from(&profile.path),
                    profile.id.clone(),
                    profile.auth.clone(),
                    profile.effective_public_url(),
                    oauth_client_secret,
                    oauth_password,
                    oauth_token_secret,
                    profile.runtime.clone(),
                )
            }
            ServiceKind::Actions => {
                let auth_type = profile.actions.auth_type.clone();
                let use_shared = profile.actions.use_shared_secrets;
                let api_key = if auth_type == "api_key" {
                    resolve_secret(&profile.id, "actions_api_key", use_shared)?
                } else {
                    None
                };
                let oauth_client_secret = if auth_type == "oauth" {
                    if use_shared {
                        resolve_secret(&profile.id, "actions_oauth_client_secret", true)?
                    } else {
                        Some(actions_oauth_secret(
                            &profile.id,
                            "actions_oauth_client_secret",
                        )?)
                    }
                } else {
                    None
                };
                let oauth_password = if auth_type == "oauth" {
                    if use_shared {
                        resolve_secret(&profile.id, "actions_oauth_password", true)?
                    } else {
                        Some(actions_oauth_secret(
                            &profile.id,
                            "actions_oauth_password",
                        )?)
                    }
                } else {
                    None
                };
                let oauth_token_secret = if auth_type == "oauth" {
                    if use_shared {
                        resolve_secret(&profile.id, "actions_oauth_token_secret", true)?
                    } else {
                        Some(actions_oauth_secret(
                            &profile.id,
                            "actions_oauth_token_secret",
                        )?)
                    }
                } else {
                    None
                };
                let public_base_url = profile.actions_public_base_url();
                let policy = PolicySettings::from_actions_config(&profile.actions);
                actions::spawn_listener(
                    &profile.id,
                    port,
                    PathBuf::from(&profile.path),
                    public_base_url,
                    auth_type,
                    api_key,
                    profile.actions.oauth_client_id.clone(),
                    oauth_client_secret,
                    oauth_password,
                    oauth_token_secret,
                    policy,
                )
            }
        };

        match spawn_result {
            Ok((shutdown, handle)) => {
                self.entries.insert(
                    key,
                    RuntimeEntry {
                        phase: RuntimePhase::Running,
                        shutdown: Some(shutdown),
                        handle: Some(handle),
                        error_message: None,
                    },
                );
            }
            Err(err) => {
                // spawn_listener can fail synchronously before the server task is
                // ever created (e.g. missing API key / OAuth secret). In that case
                // serve() never runs, so nothing writes to the stderr log and the
                // failure was previously invisible in the log viewer. Record it here.
                append_profile_log(
                    &profile.id,
                    stderr_log_name(kind),
                    &format!("[start] {}启动失败：{err}", service_label(kind).trim()),
                );
                self.entries.insert(
                    key,
                    RuntimeEntry {
                        phase: RuntimePhase::Error,
                        shutdown: None,
                        handle: None,
                        error_message: Some(err.to_string()),
                    },
                );
            }
        }

        Ok(self.status(profile, kind))
    }

    fn stop(&mut self, profile: &WorkspaceProfile, kind: ServiceKind) -> AppResult<RuntimeStatusDto> {
        self.stop_internal(&profile.id, kind);
        Ok(self.status(profile, kind))
    }

    /// Stop the current service (if running), then immediately start a new one.
    /// This is the canonical "restart" — used when the user regenerates a key or
    /// toggles the shared-secret switch, so the listener picks up the new value.
    ///
    /// stop_internal sends the graceful-shutdown signal but the OS port may not
    /// be freed instantly (the old listener's socket is closed on the tokio
    /// event loop). We retry `start` with a short back-off to smooth over this
    /// window.
    fn restart(&mut self, profile: &WorkspaceProfile, kind: ServiceKind) -> AppResult<RuntimeStatusDto> {
        self.stop_internal(&profile.id, kind);
        // Give the old listener a moment to release the port (≤200 ms).
        for _ in 0..4 {
            let port = port_for(profile, kind);
            if platform()
                .find_pid_listening_on_port(port)
                .ok()
                .flatten()
                .is_none()
            {
                return self.start(profile, kind);
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        // Last attempt — if it still fails, return the error from start.
        self.start(profile, kind)
    }

    fn stop_internal(&mut self, workspace_id: &str, kind: ServiceKind) {
        let key = (workspace_id.to_string(), kind);
        let Some(mut entry) = self.entries.remove(&key) else {
            return;
        };

        entry.phase = RuntimePhase::Stopping;
        if let Some(shutdown) = entry.shutdown.take() {
            let _ = shutdown.send(());
        }
        if let Some(handle) = entry.handle.take() {
            // stop_internal runs on a tokio worker thread when reached via the
            // async stop_runtime / stop_actions_runtime commands. Calling
            // block_on here panics with "Cannot start a runtime from within a
            // runtime", which used to make the Stop button silently no-op. The
            // graceful-shutdown signal was already sent above, so we just await
            // the listener's completion on a detached task to free the port.
            tauri::async_runtime::spawn(async move {
                let _ = tokio::time::timeout(Duration::from_secs(3), handle).await;
            });
        }
    }

    fn refresh(&mut self, profile: &WorkspaceProfile, kind: ServiceKind) {
        let port = port_for(profile, kind);
        let runtime_listening = platform()
            .find_pid_listening_on_port(port)
            .map(|pid| pid.is_some())
            .unwrap_or(false);

        let tunnel_kind = match kind {
            ServiceKind::Mcp => TunnelServiceKind::Mcp,
            ServiceKind::Actions => TunnelServiceKind::Actions,
        };

        tauri::async_runtime::block_on(cleanup_orphan_for_runtime(
            profile,
            tunnel_kind,
            runtime_listening,
        ));
    }
}

fn port_for(profile: &WorkspaceProfile, kind: ServiceKind) -> u16 {
    match kind {
        ServiceKind::Mcp => profile.runtime.local_port,
        ServiceKind::Actions => profile.actions.local_port,
    }
}

fn endpoints(profile: &WorkspaceProfile, kind: ServiceKind) -> (String, String) {
    match kind {
        ServiceKind::Mcp => (profile.local_endpoint(), profile.public_endpoint()),
        ServiceKind::Actions => (
            profile.actions_local_base_url(),
            profile.actions_openapi_url(),
        ),
    }
}

fn public_message_for(profile: &WorkspaceProfile, kind: ServiceKind) -> String {
    match kind {
        ServiceKind::Mcp => profile.effective_public_url(),
        ServiceKind::Actions => profile.actions_effective_public_url(),
    }
}

fn service_label(kind: ServiceKind) -> &'static str {
    match kind {
        ServiceKind::Mcp => "本地 MCP ",
        ServiceKind::Actions => "本地 Actions ",
    }
}

fn stderr_log_name(kind: ServiceKind) -> &'static str {
    match kind {
        ServiceKind::Mcp => "stderr.log",
        ServiceKind::Actions => "actions-stderr.log",
    }
}

/// Resolve a secret from the shared pool or per-workspace keyring.
fn resolve_secret(
    profile_id: &str,
    key: &str,
    use_shared: bool,
) -> AppResult<Option<String>> {
    if use_shared {
        SecretStore::get_shared(key)
    } else {
        SecretStore::get(profile_id, key)
    }
}

fn actions_oauth_secret(profile_id: &str, key: &str) -> AppResult<String> {
    match SecretStore::get(profile_id, key)? {
        Some(value) if !value.is_empty() => Ok(value),
        _ => SecretStore::regenerate(profile_id, key),
    }
}
