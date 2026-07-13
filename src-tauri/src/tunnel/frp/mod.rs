mod client;

use crate::settings::AppSettings;
#[allow(unused_imports)]
use crate::settings::FrpProfile;
use crate::workspace::WorkspaceProfile;

use super::TunnelServiceKind;

pub use client::{resolve_frpc, spawn_frpc, FrpcHandle};
pub(crate) use client::{cached_frpc_path, download_frpc_to_cache};

const FRP_VERSION: &str = "0.61.2";
pub(crate) const VERSION: &str = FRP_VERSION;

#[allow(dead_code)]
pub(crate) fn frp_version() -> &'static str {
    FRP_VERSION
}

/// FRP proxy snippet for the MCP listener (`profile.tunnel` + `profile.runtime`).
#[allow(dead_code)]
pub fn mcp_frp_snippet(profile: &WorkspaceProfile, settings: &AppSettings) -> String {
    frp_snippet(profile, TunnelServiceKind::Mcp, settings)
}

/// FRP proxy snippet for the Actions listener (`profile.actions`).
#[allow(dead_code)]
pub fn actions_frp_snippet(profile: &WorkspaceProfile, settings: &AppSettings) -> String {
    frp_snippet(profile, TunnelServiceKind::Actions, settings)
}

pub fn frp_snippet(
    profile: &WorkspaceProfile,
    kind: TunnelServiceKind,
    settings: &AppSettings,
) -> String {
    let config = frp_server_config(profile, kind, settings, None);
    build_proxy_snippet(&config.proxy)
}

pub(crate) struct FrpProxyConfig {
    pub proxy_name: String,
    pub local_port: u16,
    pub subdomain: String,
}

pub(crate) struct FrpServerConfig {
    pub server_addr: String,
    pub server_port: u16,
    pub token: Option<String>,
    pub proxy: FrpProxyConfig,
}

#[allow(dead_code)]
pub fn frp_public_url(
    profile: &WorkspaceProfile,
    kind: TunnelServiceKind,
    settings: &AppSettings,
) -> String {
    let config = frp_server_config(profile, kind, settings, None);
    if config.server_addr.is_empty() || config.proxy.subdomain.trim().is_empty() {
        return String::new();
    }
    format!(
        "https://{}.{}",
        config.proxy.subdomain.trim(),
        config.server_addr.trim().trim_end_matches('/')
    )
}

pub fn frp_server_config(
    profile: &WorkspaceProfile,
    kind: TunnelServiceKind,
    settings: &AppSettings,
    token_override: Option<String>,
) -> FrpServerConfig {
    let proxy = frp_proxy_config(profile, kind);
    let (profile_id, server_addr, server_port) = match kind {
        TunnelServiceKind::Mcp => (
            profile.tunnel.frp_profile_id.as_str(),
            profile.tunnel.frp_server.clone(),
            profile.tunnel.frp_server_port,
        ),
        TunnelServiceKind::Actions => (
            profile.actions.frp_profile_id.as_str(),
            profile.actions.frp_server.clone(),
            profile.actions.frp_server_port,
        ),
    };

    let (server_addr, server_port) = if let Some(frp_profile) = settings.find_frp_profile(profile_id) {
        (frp_profile.server.clone(), frp_profile.server_port)
    } else {
        (server_addr, server_port)
    };

    let token = token_override.or_else(|| resolve_frp_token(profile_id, profile, kind, settings));

    FrpServerConfig {
        server_addr,
        server_port,
        token,
        proxy,
    }
}

fn resolve_frp_token(
    profile_id: &str,
    workspace: &WorkspaceProfile,
    kind: TunnelServiceKind,
    settings: &AppSettings,
) -> Option<String> {
    if !profile_id.trim().is_empty() {
        if let Ok(Some(token)) = crate::secret::SecretStore::get_app("frp_profile_token", profile_id)
        {
            if !token.trim().is_empty() {
                return Some(token);
            }
        }
    }

    let workspace_key = match kind {
        TunnelServiceKind::Mcp => "frp_token",
        TunnelServiceKind::Actions => "actions_frp_token",
    };
    if let Ok(Some(token)) = crate::secret::SecretStore::get(&workspace.id, workspace_key) {
        if !token.trim().is_empty() {
            return Some(token);
        }
    }

    // Manual inline server: reuse token from a global profile with the same host.
    let inline_server = match kind {
        TunnelServiceKind::Mcp => workspace.tunnel.frp_server.as_str(),
        TunnelServiceKind::Actions => workspace.actions.frp_server.as_str(),
    };
    let inline_server = inline_server.trim();
    if !inline_server.is_empty() {
        for profile in &settings.frp_profiles {
            if profile.server.trim().eq_ignore_ascii_case(inline_server) {
                if let Ok(Some(token)) =
                    crate::secret::SecretStore::get_app("frp_profile_token", &profile.id)
                {
                    if !token.trim().is_empty() {
                        return Some(token);
                    }
                }
            }
        }
    }

    None
}

pub fn build_frpc_toml(config: &FrpServerConfig) -> String {
    let mut lines = vec![
        format!("serverAddr = \"{}\"", config.server_addr.trim()),
        format!("serverPort = {}", config.server_port),
        String::new(),
    ];
    if let Some(token) = config.token.as_ref().filter(|t| !t.trim().is_empty()) {
        lines.push("auth.method = \"token\"".to_string());
        lines.push(format!("auth.token = \"{}\"", token.trim()));
        lines.push(String::new());
    }
    lines.push(build_proxy_snippet(&config.proxy));
    lines.join("\n")
}

fn frp_proxy_config(profile: &WorkspaceProfile, kind: TunnelServiceKind) -> FrpProxyConfig {
    let slug = workspace_slug(&profile.name);
    match kind {
        TunnelServiceKind::Mcp => FrpProxyConfig {
            proxy_name: format!("{slug}-mcp"),
            local_port: profile.runtime.local_port,
            subdomain: profile.tunnel.frp_subdomain.clone(),
        },
        TunnelServiceKind::Actions => FrpProxyConfig {
            proxy_name: format!("{slug}-actions"),
            local_port: profile.actions.local_port,
            subdomain: profile.actions.frp_subdomain.clone(),
        },
    }
}

fn build_proxy_snippet(proxy: &FrpProxyConfig) -> String {
    [
        "[[proxies]]".to_string(),
        format!("name = \"{}\"", proxy.proxy_name),
        "type = \"http\"".to_string(),
        "localIP = \"127.0.0.1\"".to_string(),
        format!("localPort = {}", proxy.local_port),
        format!("subdomain = \"{}\"", proxy.subdomain.trim()),
    ]
    .join("\n")
}

fn workspace_slug(name: &str) -> String {
    let slug: String = name
        .to_lowercase()
        .chars()
        .map(|c| if c.is_whitespace() { '-' } else { c })
        .collect();
    if slug.is_empty() || slug.chars().all(|c| c == '-') {
        "workspace".to_string()
    } else {
        slug
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::FrpProfile;
    use crate::workspace::WorkspaceProfile;

    #[test]
    fn mcp_snippet_uses_tunnel_subdomain() {
        let mut profile = WorkspaceProfile::new("/tmp/demo".into(), Some("Demo WS".into()));
        profile.tunnel.frp_subdomain = "demo-mcp".into();
        profile.runtime.local_port = 28766;
        let settings = AppSettings {
            frp_profiles: vec![FrpProfile {
                id: "p1".into(),
                name: "Main".into(),
                server: "frp.example.com".into(),
                server_port: 7000,
            }],
            ..AppSettings::default()
        };
        profile.tunnel.frp_profile_id = "p1".into();

        let snippet = mcp_frp_snippet(&profile, &settings);
        assert!(snippet.contains("name = \"demo-ws-mcp\""));
        assert!(snippet.contains("localPort = 28766"));
        assert!(snippet.contains("subdomain = \"demo-mcp\""));
    }

    #[test]
    fn build_frpc_toml_uses_global_profile_server() {
        let mut profile = WorkspaceProfile::new("/tmp/demo".into(), Some("Demo".into()));
        profile.tunnel.frp_subdomain = "demo".into();
        profile.tunnel.frp_profile_id = "p1".into();
        let settings = AppSettings {
            frp_profiles: vec![FrpProfile {
                id: "p1".into(),
                name: "Main".into(),
                server: "frp.example.com".into(),
                server_port: 7000,
            }],
            ..AppSettings::default()
        };
        let config = frp_server_config(
            &profile,
            TunnelServiceKind::Mcp,
            &settings,
            Some("secret".into()),
        );
        let toml = build_frpc_toml(&config);
        assert!(toml.contains("serverAddr = \"frp.example.com\""));
        assert!(toml.contains("auth.token = \"secret\""));
    }

    #[test]
    fn resolve_token_from_matching_global_profile_when_manual_server() {
        let mut profile = WorkspaceProfile::new("/tmp/demo".into(), Some("Demo".into()));
        profile.tunnel.frp_server = "frp.example.com".into();
        profile.tunnel.frp_subdomain = "demo".into();
        let settings = AppSettings {
            frp_profiles: vec![FrpProfile {
                id: "p1".into(),
                name: "Main".into(),
                server: "frp.example.com".into(),
                server_port: 7000,
            }],
            ..AppSettings::default()
        };
        crate::secret::SecretStore::set_app("frp_profile_token", "p1", "shared-token").unwrap();
        let config = frp_server_config(&profile, TunnelServiceKind::Mcp, &settings, None);
        assert_eq!(config.token.as_deref(), Some("shared-token"));
    }
}
