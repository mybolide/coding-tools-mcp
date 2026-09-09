use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::settings::AppSettings;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceProfile {
    pub id: String,
    pub name: String,
    pub path: String,
    pub tunnel: TunnelConfig,
    pub auth: AuthConfig,
    pub runtime: RuntimeConfig,
    #[serde(default)]
    pub actions: ActionsConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TunnelConfig {
    #[serde(rename = "type", default = "default_tunnel_type")]
    pub tunnel_type: String,
    #[serde(default)]
    pub public_url: String,
    #[serde(default)]
    pub frp_server: String,
    #[serde(default)]
    pub frp_subdomain: String,
    #[serde(default)]
    pub frp_profile_id: String,
    #[serde(default = "default_frp_server_port")]
    pub frp_server_port: u16,
    #[serde(default = "default_cloudflare_mode")]
    pub cloudflare_mode: String,
    /// When true, start cloudflared with `--protocol http2` instead of default QUIC.
    #[serde(default = "default_cloudflare_http2")]
    pub cloudflare_http2: bool,
    /// When true, apply global proxy from Settings → General when starting the tunnel.
    #[serde(default = "default_use_proxy")]
    pub use_proxy: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthConfig {
    #[serde(rename = "type", default = "default_auth_type")]
    pub auth_type: String,
    #[serde(default = "default_oauth_client_id")]
    pub oauth_client_id: String,
    #[serde(default)]
    pub use_shared_secrets: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeConfig {
    #[serde(default = "default_mcp_port")]
    pub local_port: u16,
    #[serde(default = "default_tool_profile")]
    pub tool_profile: String,
    #[serde(default = "default_permission_mode")]
    pub permission_mode: String,
    #[serde(default)]
    pub runtime_command: String,
    /// Workspace execution policy shared by MCP clients.
    #[serde(default = "default_allowed_commands")]
    pub allowed_commands: String,
    #[serde(default = "default_workspace_local_entries")]
    pub workspace_local_entries: bool,
    #[serde(default = "default_workspace_script_extensions")]
    pub workspace_script_extensions: String,
    /// Local MCP servers started by this workspace's public MCP runtime.
    /// They are never reachable directly from the public network.
    #[serde(default)]
    pub upstream_mcps: Vec<UpstreamMcpConfig>,
}

/// A deliberately narrow, workspace-owned MCP configuration. HTTP endpoints
/// are only contacted after the workspace owner saves them; remote tool calls
/// can never supply a URL or override the configured request headers.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UpstreamMcpConfig {
    #[serde(default = "default_upstream_id")]
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub enabled: bool,
    #[serde(rename = "type", alias = "transport", default = "default_upstream_transport")]
    pub transport: String,
    #[serde(default)]
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: std::collections::BTreeMap<String, String>,
    /// Streamable HTTP endpoint. Required only when `transport` is
    /// `streamableHttp` and ignored by stdio transports.
    #[serde(default)]
    pub url: String,
    /// Optional headers injected into requests to this one explicit HTTP
    /// endpoint. They never appear in the public tool catalogue or logs.
    #[serde(default)]
    pub headers: std::collections::BTreeMap<String, String>,
    #[serde(default)]
    pub tool_prefix: String,
    #[serde(default)]
    pub allowed_tools: Vec<String>,
    /// Explicitly hidden upstream tool names. New configurations expose all
    /// discovered tools by default; this list is the opt-out control persisted
    /// by the workspace UI. A non-empty legacy `allowed_tools` list continues
    /// to take precedence until the user saves the new controls.
    #[serde(default)]
    pub disabled_tools: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionsConfig {
    #[serde(default)]
    pub public_url: String,
    #[serde(default = "default_tunnel_type")]
    pub tunnel_type: String,
    #[serde(default)]
    pub frp_server: String,
    #[serde(default)]
    pub frp_subdomain: String,
    #[serde(default)]
    pub frp_profile_id: String,
    #[serde(default = "default_frp_server_port")]
    pub frp_server_port: u16,
    #[serde(default = "default_cloudflare_mode")]
    pub cloudflare_mode: String,
    #[serde(default)]
    pub cloudflare_token: String,
    #[serde(default = "default_cloudflare_http2")]
    pub cloudflare_http2: bool,
    #[serde(default = "default_use_proxy")]
    pub use_proxy: bool,
    #[serde(default = "default_actions_port")]
    pub local_port: u16,
    #[serde(default = "default_permission_mode")]
    pub permission_mode: String,
    #[serde(default)]
    pub runtime_command: String,
    #[serde(default = "default_actions_auth_type")]
    pub auth_type: String,
    #[serde(default = "default_actions_oauth_client_id")]
    pub oauth_client_id: String,
    #[serde(default)]
    pub oauth_scopes: String,
    #[serde(default = "default_allowed_commands")]
    pub allowed_commands: String,
    #[serde(default = "default_max_patch_bytes")]
    pub max_patch_bytes: u32,
    #[serde(default)]
    pub use_shared_secrets: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeStatusDto {
    pub state: String,
    pub pid: Option<u32>,
    pub local_message: String,
    pub public_message: String,
    pub local_endpoint: String,
    pub public_endpoint: String,
}

fn default_tunnel_type() -> String {
    "frp".to_string()
}

fn default_cloudflare_mode() -> String {
    "quick".to_string()
}

fn default_cloudflare_http2() -> bool {
    true
}

fn default_use_proxy() -> bool {
    true
}

fn default_auth_type() -> String {
    "oauth".to_string()
}

fn default_frp_server_port() -> u16 {
    7000
}

fn default_actions_auth_type() -> String {
    "api_key".to_string()
}

fn default_actions_oauth_client_id() -> String {
    format!(
        "chatgpt-actions-{}",
        &uuid::Uuid::new_v4().to_string()[..12]
    )
}

fn default_oauth_client_id() -> String {
    format!("chatgpt-client-{}", &uuid::Uuid::new_v4().to_string()[..12])
}

fn default_mcp_port() -> u16 {
    28766
}

fn default_actions_port() -> u16 {
    8787
}

fn default_tool_profile() -> String {
    "core".to_string()
}

fn default_permission_mode() -> String {
    "trusted".to_string()
}

fn default_allowed_commands() -> String {
    "pytest,python,python3,npm,npx,node,pnpm,yarn,make,mvn,mvnw,gradle,gradlew,cargo,go,ruff,mypy,eslint,tsc,git,cmd,powershell,pwsh".to_string()
}

fn default_workspace_local_entries() -> bool {
    true
}

fn default_workspace_script_extensions() -> String {
    ".exe,.bat,.cmd,.ps1".to_string()
}

fn default_max_patch_bytes() -> u32 {
    200_000
}

fn default_upstream_id() -> String {
    uuid::Uuid::new_v4().to_string().replace('-', "")
}

fn default_upstream_transport() -> String {
    "stdio".to_string()
}

impl Default for TunnelConfig {
    fn default() -> Self {
        Self {
            tunnel_type: default_tunnel_type(),
            public_url: String::new(),
            frp_server: String::new(),
            frp_subdomain: String::new(),
            frp_profile_id: String::new(),
            frp_server_port: default_frp_server_port(),
            cloudflare_mode: default_cloudflare_mode(),
            cloudflare_http2: default_cloudflare_http2(),
            use_proxy: default_use_proxy(),
        }
    }
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            auth_type: default_auth_type(),
            oauth_client_id: default_oauth_client_id(),
            use_shared_secrets: false,
        }
    }
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            local_port: default_mcp_port(),
            tool_profile: default_tool_profile(),
            permission_mode: default_permission_mode(),
            runtime_command: String::new(),
            allowed_commands: default_allowed_commands(),
            workspace_local_entries: default_workspace_local_entries(),
            workspace_script_extensions: default_workspace_script_extensions(),
            upstream_mcps: Vec::new(),
        }
    }
}

impl UpstreamMcpConfig {
    pub fn public_tool_name(&self, original_name: &str) -> String {
        format!("{}__{original_name}", self.tool_prefix)
    }

    pub fn uses_legacy_allowlist(&self) -> bool {
        !self.allowed_tools.is_empty()
    }

    pub fn exposes_tool(&self, original_name: &str) -> bool {
        if self.uses_legacy_allowlist() {
            self.allowed_tools.iter().any(|tool| tool == original_name)
        } else {
            !self.disabled_tools.iter().any(|tool| tool == original_name)
        }
    }
}

/// Validate configuration that will be persisted and later used to start an
/// explicit stdio process or Streamable HTTP endpoint. This validates shape and
/// collisions only; reachability is checked while the MCP runtime starts.
pub fn validate_upstream_mcps(configs: &[UpstreamMcpConfig]) -> Result<(), String> {
    let mut ids = HashSet::new();
    let mut names = HashSet::new();
    let mut prefixes = HashSet::new();

    for config in configs {
        if config.id.trim().is_empty() || !ids.insert(config.id.trim()) {
            return Err("每个本地 MCP 都需要唯一 ID".to_string());
        }
        if config.name.trim().is_empty() || !names.insert(config.name.trim()) {
            return Err("每个本地 MCP 都需要唯一名称".to_string());
        }
        match config.transport.as_str() {
            "stdio" => {
                if config.enabled && config.command.trim().is_empty() {
                    return Err(format!("本地 MCP「{}」缺少启动命令", config.name));
                }
            }
            "streamableHttp" => {
                if config.enabled && !valid_streamable_http_url(&config.url) {
                    return Err(format!(
                        "本地 MCP「{}」需要有效的 http 或 https Streamable HTTP 地址",
                        config.name
                    ));
                }
            }
            _ => {
                return Err(format!(
                    "本地 MCP「{}」的传输类型必须是 stdio 或 streamableHttp",
                    config.name
                ));
            }
        }
        if !valid_tool_prefix(&config.tool_prefix) {
            return Err(format!(
                "本地 MCP「{}」的工具前缀必须由小写字母、数字或短横线组成",
                config.name
            ));
        }
        if config.enabled && !prefixes.insert(config.tool_prefix.as_str()) {
            return Err(format!("已启用的本地 MCP 工具前缀重复：{}", config.tool_prefix));
        }
        let mut allowed = HashSet::new();
        for tool in &config.allowed_tools {
            let trimmed = tool.trim();
            if trimmed.is_empty() || !allowed.insert(trimmed) {
                return Err(format!("本地 MCP「{}」的工具白名单包含重复或空名称", config.name));
            }
        }
        let mut disabled = HashSet::new();
        for tool in &config.disabled_tools {
            let trimmed = tool.trim();
            if trimmed.is_empty() || !disabled.insert(trimmed) {
                return Err(format!("本地 MCP「{}」的关闭工具列表包含重复或空名称", config.name));
            }
        }
        for (key, value) in &config.env {
            if key.trim().is_empty()
                || key.contains('=')
                || key.contains('\0')
                || value.contains('\0')
            {
                return Err(format!("本地 MCP「{}」包含无效环境变量名称", config.name));
            }
        }
        for (key, value) in &config.headers {
            if reqwest::header::HeaderName::from_bytes(key.trim().as_bytes()).is_err()
                || value.contains('\r')
                || value.contains('\n')
                || value.parse::<reqwest::header::HeaderValue>().is_err()
            {
                return Err(format!("本地 MCP「{}」包含无效 HTTP 请求头", config.name));
            }
        }
    }
    Ok(())
}

fn valid_streamable_http_url(value: &str) -> bool {
    let Ok(url) = reqwest::Url::parse(value.trim()) else {
        return false;
    };
    matches!(url.scheme(), "http" | "https") && url.host_str().is_some()
}

fn valid_tool_prefix(prefix: &str) -> bool {
    !prefix.is_empty()
        && prefix
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
}

impl Default for ActionsConfig {
    fn default() -> Self {
        Self {
            public_url: String::new(),
            tunnel_type: default_tunnel_type(),
            frp_server: String::new(),
            frp_subdomain: String::new(),
            frp_profile_id: String::new(),
            frp_server_port: default_frp_server_port(),
            cloudflare_mode: default_cloudflare_mode(),
            cloudflare_token: String::new(),
            cloudflare_http2: default_cloudflare_http2(),
            use_proxy: default_use_proxy(),
            local_port: default_actions_port(),
            permission_mode: default_permission_mode(),
            runtime_command: String::new(),
            auth_type: default_actions_auth_type(),
            oauth_client_id: default_actions_oauth_client_id(),
            oauth_scopes: String::new(),
            allowed_commands: default_allowed_commands(),
            max_patch_bytes: default_max_patch_bytes(),
            use_shared_secrets: false,
        }
    }
}

#[allow(dead_code)]
impl WorkspaceProfile {
    pub fn new(path: String, name: Option<String>) -> Self {
        let cleaned = path.trim_end_matches(['\\', '/']).to_string();
        let label = name.unwrap_or_else(|| {
            cleaned
                .replace('\\', "/")
                .split('/')
                .next_back()
                .unwrap_or("工作区")
                .to_string()
        });
        Self {
            id: uuid::Uuid::new_v4().to_string().replace('-', ""),
            name: label,
            path: cleaned,
            tunnel: TunnelConfig::default(),
            auth: AuthConfig::default(),
            runtime: RuntimeConfig::default(),
            actions: ActionsConfig::default(),
        }
    }

    pub fn local_endpoint(&self) -> String {
        format!("http://127.0.0.1:{}/mcp", self.runtime.local_port)
    }

    pub fn effective_public_url(&self) -> String {
        self.effective_public_url_with(&AppSettings::load_or_default())
    }

    pub fn effective_public_url_with(&self, settings: &AppSettings) -> String {
        computed_public_url(
            &self.tunnel.tunnel_type,
            &self.tunnel.frp_server,
            &self.tunnel.frp_subdomain,
            &self.tunnel.public_url,
            &self.tunnel.frp_profile_id,
            settings,
        )
    }

    pub fn public_endpoint(&self) -> String {
        let base = self.effective_public_url();
        if base.is_empty() {
            return String::new();
        }
        format!("{}/mcp", base.trim_end_matches('/'))
    }

    pub fn actions_local_base_url(&self) -> String {
        format!("http://127.0.0.1:{}", self.actions.local_port)
    }

    pub fn actions_effective_public_url(&self) -> String {
        self.actions_effective_public_url_with(&AppSettings::load_or_default())
    }

    pub fn actions_effective_public_url_with(&self, settings: &AppSettings) -> String {
        computed_public_url(
            &self.actions.tunnel_type,
            &self.actions.frp_server,
            &self.actions.frp_subdomain,
            &self.actions.public_url,
            &self.actions.frp_profile_id,
            settings,
        )
    }

    pub fn actions_openapi_url(&self) -> String {
        let base = self.actions_public_base_url();
        if base.is_empty() {
            return String::new();
        }
        format!("{}/openapi.json", base.trim_end_matches('/'))
    }

    pub fn actions_privacy_url(&self) -> String {
        let base = self.actions_public_base_url();
        if base.is_empty() {
            return String::new();
        }
        format!("{}/privacy", base.trim_end_matches('/'))
    }

    pub fn actions_oauth_authorization_url(&self) -> String {
        let base = self.actions_public_base_url();
        if base.is_empty() {
            return String::new();
        }
        format!("{}/oauth/authorize", base.trim_end_matches('/'))
    }

    pub fn actions_oauth_token_url(&self) -> String {
        let base = self.actions_public_base_url();
        if base.is_empty() {
            return String::new();
        }
        format!("{}/oauth/token", base.trim_end_matches('/'))
    }

    /// Public URL for GPT schema import; falls back to localhost when no tunnel is configured.
    pub fn actions_public_base_url(&self) -> String {
        let public = self.actions_effective_public_url();
        if public.is_empty() {
            self.actions_local_base_url()
        } else {
            public
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{validate_upstream_mcps, UpstreamMcpConfig};

    fn example_upstream() -> UpstreamMcpConfig {
        UpstreamMcpConfig {
            id: "example".into(),
            name: "Example MCP".into(),
            enabled: true,
            transport: "stdio".into(),
            command: "example-mcp".into(),
            args: vec!["serve".into()],
            env: Default::default(),
            url: String::new(),
            headers: Default::default(),
            tool_prefix: "example".into(),
            allowed_tools: vec!["status".into()],
            disabled_tools: Vec::new(),
        }
    }

    #[test]
    fn upstream_stdio_configuration_is_valid() {
        assert!(validate_upstream_mcps(&[example_upstream()]).is_ok());
    }

    #[test]
    fn streamable_http_configuration_is_valid() {
        let mut config = example_upstream();
        config.transport = "streamableHttp".into();
        config.command.clear();
        config.url = "https://127.0.0.1:3000/mcp".into();
        config.headers.insert("Authorization".into(), "Bearer token".into());
        assert!(validate_upstream_mcps(&[config]).is_ok());
    }

    #[test]
    fn streamable_http_requires_http_url() {
        let mut config = example_upstream();
        config.transport = "streamableHttp".into();
        config.command.clear();
        config.url = "file:///local/mcp".into();
        assert!(validate_upstream_mcps(&[config]).is_err());
    }

    #[test]
    fn upstream_rejects_invalid_http_header() {
        let mut config = example_upstream();
        config.headers.insert("bad header".into(), "value".into());
        assert!(validate_upstream_mcps(&[config]).is_err());
    }

    #[test]
    fn enabled_upstreams_cannot_share_a_prefix() {
        let mut duplicate = example_upstream();
        duplicate.id = "other".into();
        duplicate.name = "Other".into();
        assert!(validate_upstream_mcps(&[example_upstream(), duplicate]).is_err());
    }

    #[test]
    fn upstream_public_name_is_namespaced() {
        assert_eq!(example_upstream().public_tool_name("status"), "example__status");
    }

    #[test]
    fn new_configurations_expose_every_discovered_tool_by_default() {
        let mut config = example_upstream();
        config.allowed_tools.clear();
        assert!(config.exposes_tool("status"));
        assert!(config.exposes_tool("build"));
    }

    #[test]
    fn disabled_tool_is_not_exposed_by_new_configuration() {
        let mut config = example_upstream();
        config.allowed_tools.clear();
        config.disabled_tools.push("build".into());
        assert!(config.exposes_tool("status"));
        assert!(!config.exposes_tool("build"));
    }

    #[test]
    fn non_empty_legacy_allowlist_remains_restrictive() {
        let config = example_upstream();
        assert!(config.exposes_tool("status"));
        assert!(!config.exposes_tool("build"));
    }
}

fn computed_public_url(
    tunnel_type: &str,
    frp_server: &str,
    frp_subdomain: &str,
    public_url: &str,
    frp_profile_id: &str,
    settings: &AppSettings,
) -> String {
    if tunnel_type == "frp" {
        let server = settings
            .find_frp_profile(frp_profile_id)
            .map(|profile| profile.server.as_str())
            .unwrap_or(frp_server);
        if !server.is_empty() && !frp_subdomain.is_empty() {
            return format!("https://{frp_subdomain}.{server}");
        }
    }
    public_url.trim_end_matches('/').to_string()
}
