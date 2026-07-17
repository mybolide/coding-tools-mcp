use std::time::Duration;

use reqwest::header::{ACCEPT, AUTHORIZATION, CONTENT_TYPE, WWW_AUTHENTICATE};
use serde::Serialize;
use serde_json::{json, Value};

use crate::secret::SecretStore;
use crate::workspace::WorkspaceProfile;

const TIMEOUT: Duration = Duration::from_secs(4);
const MCP_PROTOCOL_VERSION: &str = "2025-06-18";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthItem {
    pub label: String,
    pub ok: bool,
    pub detail: String,
    pub hint: String,
}

fn http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(TIMEOUT)
        .build()
        .expect("failed to build HTTP client")
}

fn format_single_value(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        other => other.to_string(),
    }
}

fn format_field_value(value: &Value) -> String {
    match value {
        Value::Array(items) => items
            .iter()
            .map(format_single_value)
            .collect::<Vec<_>>()
            .join(" / "),
        other => format_single_value(other),
    }
}

fn field_has_value(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Array(items) => !items.is_empty(),
        Value::String(value) => !value.is_empty(),
        _ => true,
    }
}

async fn check_url(client: &reqwest::Client, url: &str) -> (bool, String) {
    if url.is_empty() {
        return (false, "URL not configured".to_string());
    }
    match client.get(url).send().await {
        Ok(response) => {
            let code = response.status().as_u16();
            let ok = matches!(code, 200 | 401 | 404);
            (ok, format!("HTTP {code}"))
        }
        Err(err) => (false, err.to_string()),
    }
}

async fn check_json_field(client: &reqwest::Client, url: &str, field: &str) -> (bool, String) {
    if url.is_empty() {
        return (false, "URL not configured".to_string());
    }
    match client.get(url).send().await {
        Ok(response) => {
            let status = response.status();
            if !status.is_success() {
                return (false, format!("HTTP {}", status.as_u16()));
            }
            match response.json::<Value>().await {
                Ok(payload) => {
                    let Some(value) = payload.get(field) else {
                        return (false, format!("HTTP {}; missing {field}", status.as_u16()));
                    };
                    if !field_has_value(value) {
                        return (false, format!("HTTP {}; empty {field}", status.as_u16()));
                    }
                    (
                        true,
                        format!("HTTP {}; {field}={}", status.as_u16(), format_field_value(value)),
                    )
                }
                Err(err) => (false, err.to_string()),
            }
        }
        Err(err) => (false, err.to_string()),
    }
}

async fn check_oauth_json_field(
    client: &reqwest::Client,
    url: &str,
    field: &str,
    auth_type: &str,
) -> (bool, String) {
    if auth_type != "oauth" {
        return (true, format!("not applicable; auth_type={auth_type}"));
    }
    check_json_field(client, url, field).await
}

fn well_known_url(base: &str, path: &str) -> String {
    if base.is_empty() {
        return String::new();
    }
    format!("{}/{}", base.trim_end_matches('/'), path)
}

fn endpoint_base(endpoint: &str) -> String {
    endpoint
        .trim_end_matches('/')
        .strip_suffix("/mcp")
        .unwrap_or(endpoint.trim_end_matches('/'))
        .trim_end_matches('/')
        .to_string()
}

fn initialize_request() -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": MCP_PROTOCOL_VERSION,
            "capabilities": {},
            "clientInfo": {
                "name": "coding-tools-mcp-health-check",
                "version": env!("CARGO_PKG_VERSION")
            }
        }
    })
}

fn initialize_protocol_version(body: &str) -> Option<String> {
    if let Ok(payload) = serde_json::from_str::<Value>(body) {
        if let Some(version) = payload
            .pointer("/result/protocolVersion")
            .and_then(Value::as_str)
        {
            return Some(version.to_string());
        }
    }

    body.lines().find_map(|line| {
        let data = line.trim().strip_prefix("data:")?.trim();
        let payload = serde_json::from_str::<Value>(data).ok()?;
        payload
            .pointer("/result/protocolVersion")
            .and_then(Value::as_str)
            .map(ToString::to_string)
    })
}

fn has_bearer_challenge(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    lower.starts_with("bearer ") && lower.contains("resource_metadata=\"")
}

async fn check_mcp_protocol(
    client: &reqwest::Client,
    url: &str,
    auth_type: &str,
    bearer_token: Option<&str>,
    bearer_token_error: Option<&str>,
) -> (bool, String) {
    if url.is_empty() {
        return (false, "URL not configured".to_string());
    }

    let mut request = client
        .post(url)
        .header(CONTENT_TYPE, "application/json")
        .header(ACCEPT, "application/json, text/event-stream")
        .json(&initialize_request());
    if let Some(token) = bearer_token {
        request = request.header(AUTHORIZATION, format!("Bearer {token}"));
    }

    let response = match request.send().await {
        Ok(response) => response,
        Err(err) => return (false, err.to_string()),
    };
    let status = response.status();
    let challenge = response
        .headers()
        .get(WWW_AUTHENTICATE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("")
        .to_string();
    let body = response.text().await.unwrap_or_default();

    if status.is_success() {
        let Some(version) = initialize_protocol_version(&body) else {
            return (
                false,
                format!("HTTP {}; initialize response is not valid MCP JSON-RPC", status.as_u16()),
            );
        };
        if auth_type == "bearer" && bearer_token.is_none() {
            return (
                false,
                format!("HTTP {}; bearer authentication was bypassed", status.as_u16()),
            );
        }
        if version != MCP_PROTOCOL_VERSION {
            return (
                false,
                format!("HTTP {}; protocolVersion={version}", status.as_u16()),
            );
        }
        return (
            true,
            format!("HTTP {}; initialize protocolVersion={version}", status.as_u16()),
        );
    }

    if status == reqwest::StatusCode::UNAUTHORIZED {
        match auth_type {
            "oauth" if has_bearer_challenge(&challenge) => {
                return (
                    true,
                    "HTTP 401; OAuth bearer challenge and resource metadata advertised".into(),
                );
            }
            "oauth" => {
                return (
                    false,
                    "HTTP 401; WWW-Authenticate/resource_metadata challenge missing".into(),
                );
            }
            "bearer" => {
                let reason = bearer_token_error.unwrap_or("Bearer token rejected");
                return (false, format!("HTTP 401; {reason}"));
            }
            _ => {}
        }
    }

    (
        false,
        format!("HTTP {}; MCP initialize rejected", status.as_u16()),
    )
}

fn bearer_probe_credentials(profile: &WorkspaceProfile) -> (Option<String>, Option<String>) {
    if profile.auth.auth_type != "bearer" {
        return (None, None);
    }
    let result = if profile.auth.use_shared_secrets {
        SecretStore::get_shared("bearer_token")
    } else {
        SecretStore::get(&profile.id, "bearer_token")
    };
    match result {
        Ok(Some(token)) => (Some(token), None),
        Ok(None) => (None, Some("Bearer token is not configured".into())),
        Err(error) => (None, Some(format!("cannot read Bearer token: {error}"))),
    }
}

pub async fn run_health_checks(profile: &WorkspaceProfile) -> Vec<HealthItem> {
    let client = http_client();
    let mcp_local = profile.local_endpoint();
    let mcp_public = profile.public_endpoint();
    let mcp_local_base = endpoint_base(&mcp_local);
    let mcp_public_base = endpoint_base(&mcp_public);
    let actions_local = profile.actions_local_base_url();
    let actions_public = profile.actions_effective_public_url();
    let actions_oauth_base = if actions_public.is_empty() {
        actions_local.clone()
    } else {
        actions_public.clone()
    };
    let mcp_local_oauth_url = well_known_url(
        &mcp_local_base,
        ".well-known/oauth-authorization-server",
    );
    let mcp_public_oauth_url = well_known_url(
        &mcp_public_base,
        ".well-known/oauth-authorization-server",
    );
    let mcp_local_protected_url =
        well_known_url(&mcp_local_base, ".well-known/oauth-protected-resource");
    let mcp_public_protected_url =
        well_known_url(&mcp_public_base, ".well-known/oauth-protected-resource");
    let actions_oauth_url = well_known_url(
        &actions_oauth_base,
        ".well-known/oauth-authorization-server",
    );
    let (bearer_token, bearer_token_error) = bearer_probe_credentials(profile);

    let (
        mcp_local_basic,
        mcp_public_basic,
        mcp_local_protocol,
        mcp_public_protocol,
        mcp_local_oauth,
        mcp_public_oauth,
        mcp_local_protected,
        mcp_public_protected,
    ) = tokio::join!(
        check_url(&client, &mcp_local),
        check_url(&client, &mcp_public),
        check_mcp_protocol(
            &client,
            &mcp_local,
            &profile.auth.auth_type,
            bearer_token.as_deref(),
            bearer_token_error.as_deref(),
        ),
        check_mcp_protocol(
            &client,
            &mcp_public,
            &profile.auth.auth_type,
            bearer_token.as_deref(),
            bearer_token_error.as_deref(),
        ),
        check_oauth_json_field(
            &client,
            &mcp_local_oauth_url,
            "token_endpoint_auth_methods_supported",
            &profile.auth.auth_type,
        ),
        check_oauth_json_field(
            &client,
            &mcp_public_oauth_url,
            "token_endpoint_auth_methods_supported",
            &profile.auth.auth_type,
        ),
        check_oauth_json_field(
            &client,
            &mcp_local_protected_url,
            "authorization_servers",
            &profile.auth.auth_type,
        ),
        check_oauth_json_field(
            &client,
            &mcp_public_protected_url,
            "authorization_servers",
            &profile.auth.auth_type,
        ),
    );

    let actions_health_url = format!("{actions_local}/health");
    let actions_openapi_local = format!("{actions_local}/openapi.json");
    let actions_openapi_public = profile.actions_openapi_url();

    let (
        actions_local_result,
        actions_openapi_local_result,
        actions_openapi_public_result,
        actions_oauth_result,
    ) = tokio::join!(
        check_url(&client, &actions_health_url),
        check_url(&client, &actions_openapi_local),
        check_url(&client, &actions_openapi_public),
        check_oauth_json_field(
            &client,
            &actions_oauth_url,
            "token_endpoint_auth_methods_supported",
            &profile.actions.auth_type,
        ),
    );

    vec![
        health_item(
            "本地 /mcp 可达性",
            mcp_local_basic.0,
            mcp_local_basic.1,
            "确认 MCP 服务已启动，端口与工作区配置一致。",
        ),
        health_item(
            "公网 /mcp 可达性",
            mcp_public_basic.0,
            mcp_public_basic.1,
            "检查隧道是否已连接，或公网 URL 是否填写正确。",
        ),
        health_item(
            "本地 MCP 协议/认证挑战",
            mcp_local_protocol.0,
            mcp_local_protocol.1,
            "本地 POST /mcp 必须完成 initialize，或返回带 resource_metadata 的 OAuth 挑战。",
        ),
        health_item(
            "公网 MCP 协议/认证挑战",
            mcp_public_protocol.0,
            mcp_public_protocol.1,
            "若 401 没有 WWW-Authenticate，GPT 无法发现 OAuth；若此项通过但仍 Session terminated，再清理并重新授权连接器。",
        ),
        health_item(
            "本地 MCP OAuth 授权元数据",
            mcp_local_oauth.0,
            mcp_local_oauth.1,
            "确认本地 OAuth 元数据完整且 token endpoint 认证方式已声明。",
        ),
        health_item(
            "公网 MCP OAuth 授权元数据",
            mcp_public_oauth.0,
            mcp_public_oauth.1,
            "MCP 认证需设为 OAuth，且公网地址可访问。",
        ),
        health_item(
            "本地 MCP OAuth 受保护资源",
            mcp_local_protected.0,
            mcp_local_protected.1,
            "确认本地 MCP 根地址与 OAuth 资源配置一致。",
        ),
        health_item(
            "公网 MCP OAuth 受保护资源",
            mcp_public_protected.0,
            mcp_public_protected.1,
            "确认公网 MCP 根地址与 OAuth 配置一致。",
        ),
        health_item(
            "本地 Actions /health",
            actions_local_result.0,
            actions_local_result.1,
            "确认 Actions 服务已启动。",
        ),
        health_item(
            "本地 Actions /openapi.json",
            actions_openapi_local_result.0,
            actions_openapi_local_result.1,
            "Actions 监听器异常时请查看 actions-stderr.log。",
        ),
        health_item(
            "公网 Actions /openapi.json",
            actions_openapi_public_result.0,
            actions_openapi_public_result.1,
            "检查 Actions 隧道与子域名配置。",
        ),
        health_item(
            "Actions OAuth 授权元数据",
            actions_oauth_result.0,
            actions_oauth_result.1,
            "Actions 认证需设为 OAuth，公网地址需可达。",
        ),
    ]
}

fn health_item(label: &str, ok: bool, detail: String, hint: &str) -> HealthItem {
    HealthItem {
        label: label.into(),
        ok,
        detail,
        hint: if ok { String::new() } else { hint.into() },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn endpoint_base_removes_mcp_suffix() {
        assert_eq!(endpoint_base("http://127.0.0.1:28766/mcp"), "http://127.0.0.1:28766");
        assert_eq!(endpoint_base("https://example.com/mcp/"), "https://example.com");
    }

    #[test]
    fn initialize_protocol_version_supports_json_and_sse() {
        let json_body = r#"{"jsonrpc":"2.0","result":{"protocolVersion":"2025-06-18"}}"#;
        assert_eq!(initialize_protocol_version(json_body).as_deref(), Some(MCP_PROTOCOL_VERSION));

        let sse_body =
            "event: message\ndata: {\"jsonrpc\":\"2.0\",\"result\":{\"protocolVersion\":\"2025-06-18\"}}\n";
        assert_eq!(initialize_protocol_version(sse_body).as_deref(), Some(MCP_PROTOCOL_VERSION));
    }

    #[test]
    fn oauth_challenge_requires_resource_metadata() {
        asser