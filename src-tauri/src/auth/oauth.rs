use serde_json::{json, Value};

use crate::workspace::AuthConfig;

impl AuthConfig {
    pub fn oauth_enabled(&self) -> bool {
        self.auth_type == "oauth"
    }

    pub fn bearer_enabled(&self) -> bool {
        self.auth_type == "bearer"
    }

    pub fn auth_enabled(&self) -> bool {
        self.auth_type != "noauth"
    }
}

pub fn oauth_base_url(public_url: &str, local_port: u16) -> String {
    let trimmed = public_url.trim_end_matches('/');
    if trimmed.is_empty() {
        format!("http://127.0.0.1:{local_port}")
    } else {
        trimmed.to_string()
    }
}

fn token_endpoint_auth_methods(client_secret: Option<&str>) -> Vec<&'static str> {
    match client_secret {
        Some(secret) if !secret.is_empty() => {
            vec!["client_secret_post", "client_secret_basic"]
        }
        _ => vec!["none"],
    }
}

pub fn authorization_server_metadata(base_url: &str, client_secret: Option<&str>) -> Value {
    let base = base_url.trim_end_matches('/');
    let methods = token_endpoint_auth_methods(client_secret);
    json!({
        "issuer": base,
        "authorization_endpoint": format!("{base}/oauth/authorize"),
        "token_endpoint": format!("{base}/oauth/token"),
        "response_types_supported": ["code"],
        "grant_types_supported": ["authorization_code"],
        "code_challenge_methods_supported": ["S256"],
        "token_endpoint_auth_methods_supported": methods,
    })
}

pub fn protected_resource_metadata(base_url: &str) -> Value {
    let base = base_url.trim_end_matches('/');
    json!({
        "resource": base,
        "authorization_servers": [base],
        "bearer_methods_supported": ["header"],
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn oauth_enabled_only_for_oauth_type() {
        let mut auth = AuthConfig::default();
        assert!(auth.oauth_enabled());
        auth.auth_type = "bearer".into();
        assert!(!auth.oauth_enabled());
        auth.auth_type = "noauth".into();
        assert!(!auth.oauth_enabled());
    }

    #[test]
    fn authorization_metadata_includes_token_auth_methods() {
        let meta = authorization_server_metadata("https://example.com", None);
        assert_eq!(
            meta["token_endpoint_auth_methods_supported"],
            json!(["none"])
        );
        let meta = authorization_server_metadata("https://example.com", Some("secret"));
        assert_eq!(
            meta["token_endpoint_auth_methods_supported"],
            json!(["client_secret_post", "client_secret_basic"])
        );
    }

    #[test]
    fn protected_resource_metadata_lists_authorization_servers() {
        let meta = protected_resource_metadata("https://example.com");
        assert_eq!(meta["authorization_servers"], json!(["https://example.com"]));
    }

    #[test]
    fn oauth_base_url_falls_back_to_localhost() {
        assert_eq!(oauth_base_url("", 28766), "http://127.0.0.1:28766");
        assert_eq!(
            oauth_base_url("https://mcp.example.com/", 28766),
            "https://mcp.example.com"
        );
    }
}
