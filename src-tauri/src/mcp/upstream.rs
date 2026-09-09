//! A small, deliberately strict MCP client for workspace-owned stdio servers.
//!
//! This module is not a generic proxy: callers can only reach a process that a
//! workspace owner configured, and only through the names discovered and
//! allowlisted when that process started.

use std::collections::{HashMap, HashSet};
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;

use reqwest::header::{ACCEPT, CONTENT_TYPE, HeaderMap, HeaderName, HeaderValue};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio::sync::Mutex;
use tokio::time::{timeout, Instant};

use crate::workspace::{validate_upstream_mcps, UpstreamMcpConfig};

const INITIALIZATION_TIMEOUT: Duration = Duration::from_secs(10);
const TOOL_CALL_TIMEOUT: Duration = Duration::from_secs(60);
const STOP_TIMEOUT: Duration = Duration::from_secs(3);

#[derive(Clone, Debug)]
struct ToolRoute {
    upstream_id: String,
    original_name: String,
}

/// Safe, UI-facing summary of an upstream tool. The full schema remains in
/// the runtime catalogue; discovery never returns command-line or environment
/// configuration to the renderer.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveredUpstreamTool {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
}

/// Owns the child processes and the immutable public catalogue prepared during
/// MCP runtime startup. Every child has its own mutex because stdio replies are
/// matched in order and many upstream servers do not support parallel calls.
pub struct UpstreamMcpManager {
    clients: HashMap<String, Arc<Mutex<UpstreamMcp>>>,
    routes: HashMap<String, ToolRoute>,
    public_tools: Vec<Value>,
}

impl UpstreamMcpManager {
    pub fn empty() -> Self {
        Self {
            clients: HashMap::new(),
            routes: HashMap::new(),
            public_tools: Vec::new(),
        }
    }

    pub async fn start(
        configs: &[UpstreamMcpConfig],
        core_tool_names: &[&str],
    ) -> Result<Self, String> {
        validate_upstream_mcps(configs)?;

        let mut manager = Self::empty();
        let mut occupied: HashSet<String> = core_tool_names
            .iter()
            .map(|name| (*name).to_string())
            .collect();

        for config in configs.iter().filter(|config| config.enabled) {
            let result = Self::start_one(&mut manager, config, &mut occupied).await;
            if let Err(error) = result {
                manager.shutdown().await;
                return Err(error);
            }
        }
        Ok(manager)
    }

    /// Starts one short-lived stdio client, reads its validated tool catalogue,
    /// then always releases the child process. This lets the configuration UI
    /// show real per-tool switches before a workspace is saved or started.
    pub async fn inspect(config: &UpstreamMcpConfig) -> Result<Vec<DiscoveredUpstreamTool>, String> {
        let mut inspected_config = config.clone();
        inspected_config.enabled = true;
        validate_upstream_mcps(&[inspected_config.clone()])?;

        let mut client = UpstreamMcp::start(&inspected_config).await?;
        let result = client.discover_tools().await.map(|tools| {
            tools
                .into_iter()
                .filter_map(|tool| {
                    Some(DiscoveredUpstreamTool {
                        name: tool.get("name")?.as_str()?.to_string(),
                        description: tool
                            .get("description")
                            .and_then(Value::as_str)
                            .unwrap_or("")
                            .to_string(),
                        input_schema: tool.get("inputSchema")?.clone(),
                    })
                })
                .collect()
        });
        client.shutdown().await;
        result
    }

    async fn start_one(
        manager: &mut Self,
        config: &UpstreamMcpConfig,
        occupied: &mut HashSet<String>,
    ) -> Result<(), String> {
        let mut client = UpstreamMcp::start(config).await?;
        let discovered = client.discover_tools().await?;
        let discovered_names: HashSet<&str> = discovered
            .iter()
            .filter_map(|tool| tool.get("name").and_then(Value::as_str))
            .collect();

        if config.uses_legacy_allowlist() {
            for allowed in &config.allowed_tools {
                if !discovered_names.contains(allowed.as_str()) {
                    return Err(format!(
                        "本地 MCP「{}」没有发现旧版白名单工具「{}」",
                        config.name, allowed
                    ));
                }
            }
        }

        for tool in discovered {
            let Some(original_name) = tool.get("name").and_then(Value::as_str).map(str::to_string)
            else {
                continue;
            };
            if !config.exposes_tool(&original_name) {
                continue;
            }
            let public_name = config.public_tool_name(&original_name);
            if !occupied.insert(public_name.clone()) {
                return Err(format!(
                    "公开工具名称冲突：{public_name}"
                ));
            }

            let mut public_tool = tool;
            public_tool["name"] = Value::String(public_name.clone());
            manager.public_tools.push(public_tool);
            manager.routes.insert(
                public_name,
                ToolRoute {
                    upstream_id: config.id.clone(),
                    original_name,
                },
            );
        }

        manager
            .clients
            .insert(config.id.clone(), Arc::new(Mutex::new(client)));
        Ok(())
    }

    pub fn public_tools(&self) -> &[Value] {
        &self.public_tools
    }

    pub fn owns_tool(&self, public_name: &str) -> bool {
        self.routes.contains_key(public_name)
    }

    pub async fn call_tool(&self, public_name: &str, arguments: Value) -> Result<Value, String> {
        let route = self
            .routes
            .get(public_name)
            .ok_or_else(|| format!("未公开的本地 MCP 工具：{public_name}"))?;
        let client = self
            .clients
            .get(&route.upstream_id)
            .ok_or_else(|| "本地 MCP 运行时不可用，请重启服务".to_string())?;
        client
            .lock()
            .await
            .call_tool(&route.original_name, arguments)
            .await
    }

    pub async fn shutdown(&self) {
        for client in self.clients.values() {
            client.lock().await.shutdown().await;
        }
    }
}

struct UpstreamMcp {
    name: String,
    transport: UpstreamTransport,
    next_request_id: u64,
}

enum UpstreamTransport {
    Stdio(StdioMcp),
    StreamableHttp(StreamableHttpMcp),
}

struct StdioMcp {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
}

struct StreamableHttpMcp {
    client: reqwest::Client,
    url: String,
    headers: HeaderMap,
    session_id: Option<HeaderValue>,
}

impl UpstreamMcp {
    async fn start(config: &UpstreamMcpConfig) -> Result<Self, String> {
        let transport = match config.transport.as_str() {
            "stdio" => UpstreamTransport::Stdio(start_stdio_transport(config).await?),
            "streamableHttp" => {
                UpstreamTransport::StreamableHttp(StreamableHttpMcp::new(config)?)
            }
            _ => return Err(format!("本地 MCP「{}」的传输类型无效", config.name)),
        };

        let mut client = Self {
            name: config.name.clone(),
            transport,
            next_request_id: 1,
        };
        client.initialize().await?;
        Ok(client)
    }

    async fn initialize(&mut self) -> Result<(), String> {
        self.request(
            "initialize",
            json!({
                "protocolVersion": "2025-06-18",
                "capabilities": {},
                "clientInfo": { "name": "coding-tools-mcp", "version": env!("CARGO_PKG_VERSION") }
            }),
            INITIALIZATION_TIMEOUT,
        )
        .await?;
        self.notify("notifications/initialized", json!({})).await
    }

    async fn discover_tools(&mut self) -> Result<Vec<Value>, String> {
        let result = self
            .request("tools/list", json!({}), INITIALIZATION_TIMEOUT)
            .await?;
        let tools = result
            .get("tools")
            .and_then(Value::as_array)
            .ok_or_else(|| format!("本地 MCP「{}」返回了无效工具目录", self.name))?;
        let mut names = HashSet::new();
        let mut validated = Vec::with_capacity(tools.len());
        for tool in tools {
            let Some(name) = tool.get("name").and_then(Value::as_str) else {
                return Err(format!("本地 MCP「{}」返回了未命名工具", self.name));
            };
            if name.trim().is_empty() || !names.insert(name) {
                return Err(format!("本地 MCP「{}」返回了重复或空工具名称", self.name));
            }
            if !tool.get("inputSchema").is_some_and(Value::is_object) {
                return Err(format!(
                    "本地 MCP「{}」的工具「{}」缺少输入 schema",
                    self.name, name
                ));
            }
            validated.push(tool.clone());
        }
        Ok(validated)
    }

    async fn call_tool(&mut self, name: &str, arguments: Value) -> Result<Value, String> {
        self.request(
            "tools/call",
            json!({ "name": name, "arguments": arguments }),
            TOOL_CALL_TIMEOUT,
        )
        .await
    }

    async fn notify(&mut self, method: &str, params: Value) -> Result<(), String> {
        let message = json!({ "jsonrpc": "2.0", "method": method, "params": params });
        match &mut self.transport {
            UpstreamTransport::Stdio(stdio) => {
                write_stdio_message(&self.name, &mut stdio.stdin, &message, INITIALIZATION_TIMEOUT)
                    .await
            }
            UpstreamTransport::StreamableHttp(http) => {
                http.send(&self.name, &message, INITIALIZATION_TIMEOUT, false)
                    .await
                    .map(|_| ())
            }
        }
    }

    async fn request(
        &mut self,
        method: &str,
        params: Value,
        deadline: Duration,
    ) -> Result<Value, String> {
        let id = self.next_request_id;
        self.next_request_id = self.next_request_id.saturating_add(1);
        let message = json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params });
        match &mut self.transport {
            UpstreamTransport::Stdio(stdio) => {
                request_stdio(
                    &self.name,
                    &mut stdio.stdin,
                    &mut stdio.stdout,
                    id,
                    &message,
                    deadline,
                )
                .await
            }
            UpstreamTransport::StreamableHttp(http) => http
                .send(&self.name, &message, deadline, true)
                .await?
                .ok_or_else(|| format!("本地 MCP「{}」返回了无结果响应", self.name)),
        }
    }

    async fn shutdown(&mut self) {
        match &mut self.transport {
            UpstreamTransport::Stdio(stdio) => {
                let _ = stdio.stdin.shutdown().await;
                if stdio.child.try_wait().ok().flatten().is_none() {
                    let _ = stdio.child.start_kill();
                    let _ = timeout(STOP_TIMEOUT, stdio.child.wait()).await;
                }
            }
            // Streamable HTTP uses an external service. Dropping the HTTP client
            // releases local session state without terminating that service.
            UpstreamTransport::StreamableHttp(_) => {}
        }
    }
}

async fn start_stdio_transport(
    config: &UpstreamMcpConfig,
) -> Result<StdioMcp, String> {
    let mut command = Command::new(&config.command);
    command
        .args(&config.args)
        .envs(&config.env)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        // Upstream stderr often contains verbose, non-protocol logs. It must
        // never reach the public MCP response or block a child on a full pipe.
        .stderr(Stdio::null())
        .kill_on_drop(true);
    #[cfg(windows)]
    command.creation_flags(windows_hidden_creation_flags());
    let mut child = command
        .spawn()
        .map_err(|_| format!("无法启动本地 MCP「{}」", config.name))?;
    let stdin = child
        .stdin
        .take()
        .ok_or_else(|| format!("本地 MCP「{}」没有标准输入通道", config.name))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| format!("本地 MCP「{}」没有标准输出通道", config.name))?;
    Ok(StdioMcp {
        child,
        stdin,
        stdout: BufReader::new(stdout),
    })
}

async fn write_stdio_message(
    name: &str,
    stdin: &mut ChildStdin,
    message: &Value,
    deadline: Duration,
) -> Result<(), String> {
    let mut bytes = serde_json::to_vec(message)
        .map_err(|_| format!("无法编码本地 MCP「{name}」请求"))?;
    bytes.push(b'\n');
    timeout(deadline, async {
        stdin.write_all(&bytes).await?;
        stdin.flush().await
    })
    .await
    .map_err(|_| format!("写入本地 MCP「{name}」请求超时"))?
    .map_err(|_| format!("写入本地 MCP「{name}」请求失败",))
}

async fn request_stdio(
    name: &str,
    stdin: &mut ChildStdin,
    stdout: &mut BufReader<ChildStdout>,
    id: u64,
    message: &Value,
    deadline: Duration,
) -> Result<Value, String> {
    write_stdio_message(name, stdin, message, deadline).await?;
    let expires_at = Instant::now() + deadline;

    loop {
        let remaining = expires_at.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(format!("本地 MCP「{name}」请求超时"));
        }
        let mut line = String::new();
        let read = timeout(remaining, stdout.read_line(&mut line))
            .await
            .map_err(|_| format!("本地 MCP「{name}」请求超时"))?
            .map_err(|_| format!("读取本地 MCP「{name}」响应失败"))?;
        if read == 0 {
            return Err(format!("本地 MCP「{name}」已停止响应"));
        }
        let response: Value = serde_json::from_str(line.trim())
            .map_err(|_| format!("本地 MCP「{name}」在标准输出写入了无效协议数据"))?;
        if response.get("id") != Some(&json!(id)) {
            // Notifications and responses for a previous request cannot be used
            // for this call. The client serializes calls, so ignore them.
            continue;
        }
        return extract_rpc_result(name, &response);
    }
}

impl StreamableHttpMcp {
    fn new(config: &UpstreamMcpConfig) -> Result<Self, String> {
        let mut headers = HeaderMap::new();
        for (name, value) in &config.headers {
            let name = HeaderName::from_bytes(name.trim().as_bytes())
                .map_err(|_| format!("本地 MCP「{}」包含无效 HTTP 请求头", config.name))?;
            let value = value
                .parse::<HeaderValue>()
                .map_err(|_| format!("本地 MCP「{}」包含无效 HTTP 请求头", config.name))?;
            headers.insert(name, value);
        }
        let client = reqwest::Client::builder()
            .build()
            .map_err(|_| format!("无法初始化本地 MCP「{}」的 HTTP 客户端", config.name))?;
        Ok(Self {
            client,
            url: config.url.trim().to_string(),
            headers,
            session_id: None,
        })
    }

    async fn send(
        &mut self,
        name: &str,
        message: &Value,
        deadline: Duration,
        expects_result: bool,
    ) -> Result<Option<Value>, String> {
        let mut headers = self.headers.clone();
        headers.insert(
            ACCEPT,
            HeaderValue::from_static("application/json, text/event-stream"),
        );
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        headers.insert(
            HeaderName::from_static("mcp-protocol-version"),
            HeaderValue::from_static("2025-06-18"),
        );
        if let Some(session_id) = &self.session_id {
            headers.insert(HeaderName::from_static("mcp-session-id"), session_id.clone());
        }

        let response = timeout(
            deadline,
            self.client
                .post(&self.url)
                .headers(headers)
                .json(message)
                .send(),
        )
        .await
        .map_err(|_| format!("本地 MCP「{name}」HTTP 请求超时"))?
        .map_err(|_| format!("无法连接本地 MCP「{name}」的 Streamable HTTP 地址"))?;

        if let Some(session_id) = response.headers().get("mcp-session-id") {
            self.session_id = Some(session_id.clone());
        }
        if !response.status().is_success() {
            return Err(format!(
                "本地 MCP「{name}」的 Streamable HTTP 请求失败（HTTP {}）",
                response.status()
            ));
        }
        if !expects_result {
            return Ok(None);
        }

        let body = timeout(deadline, response.text())
            .await
            .map_err(|_| format!("读取本地 MCP「{name}」HTTP 响应超时"))?
            .map_err(|_| format!("读取本地 MCP「{name}」HTTP 响应失败"))?;
        let id = message.get("id").and_then(Value::as_u64);
        let response = parse_streamable_http_response(name, &body, id)?;
        Ok(Some(extract_rpc_result(name, &response)?))
    }
}

fn parse_streamable_http_response(
    name: &str,
    body: &str,
    expected_id: Option<u64>,
) -> Result<Value, String> {
    if let Ok(response) = serde_json::from_str::<Value>(body.trim()) {
        return validate_http_response_id(name, response, expected_id);
    }

    for line in body.lines() {
        let Some(data) = line.trim_start().strip_prefix("data:") else {
            continue;
        };
        let Ok(response) = serde_json::from_str::<Value>(data.trim()) else {
            continue;
        };
        if response.get("id").and_then(Value::as_u64) == expected_id {
            return Ok(response);
        }
    }
    Err(format!(
        "本地 MCP「{name}」返回了无效的 Streamable HTTP 协议响应"
    ))
}

fn validate_http_response_id(
    name: &str,
    response: Value,
    expected_id: Option<u64>,
) -> Result<Value, String> {
    if response.get("id").and_then(Value::as_u64) != expected_id {
        return Err(format!("本地 MCP「{name}」返回了不匹配的 HTTP 响应"));
    }
    Ok(response)
}

fn extract_rpc_result(name: &str, response: &Value) -> Result<Value, String> {
    if response.get("error").is_some() {
        return Err(format!("本地 MCP「{name}」返回了工具错误"));
    }
    response
        .get("result")
        .cloned()
        .ok_or_else(|| format!("本地 MCP「{name}」返回了无结果响应"))
}

#[cfg(windows)]
fn windows_hidden_creation_flags() -> u32 {
    const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    CREATE_NEW_PROCESS_GROUP | CREATE_NO_WINDOW
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::fs;
    use std::sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    };

    use axum::{
        extract::State,
        http::{header::CONTENT_TYPE, HeaderMap, HeaderValue},
        routing::post,
        Json, Router,
    };
    use serde_json::{json, Value};
    use tokio::net::TcpListener;

    use super::UpstreamMcpManager;
    use crate::workspace::UpstreamMcpConfig;

    fn config(prefix: &str, allowed_tools: Vec<&str>) -> UpstreamMcpConfig {
        UpstreamMcpConfig {
            id: prefix.to_string(),
            name: prefix.to_string(),
            enabled: true,
            transport: "stdio".to_string(),
            command: "missing-program".to_string(),
            args: Vec::new(),
            env: BTreeMap::new(),
            url: String::new(),
            headers: BTreeMap::new(),
            tool_prefix: prefix.to_string(),
            allowed_tools: allowed_tools.into_iter().map(str::to_string).collect(),
            disabled_tools: Vec::new(),
        }
    }

    #[test]
    fn empty_manager_exposes_no_routes() {
        let manager = UpstreamMcpManager::empty();
        assert!(manager.public_tools().is_empty());
        assert!(!manager.owns_tool("example__status"));
    }

    #[test]
    fn invalid_prefix_is_rejected_before_process_start() {
        let invalid = config("Bad Prefix", vec!["tool"]);
        let result = tauri::async_runtime::block_on(UpstreamMcpManager::start(&[invalid], &[]));
        assert!(result.is_err());
        assert!(result.err().expect("invalid config").contains("工具前缀"));
    }

    #[cfg(windows)]
    #[test]
    fn upstream_mcp_creation_flags_hide_console_windows() {
        assert_eq!(
            super::windows_hidden_creation_flags(),
            0x0000_0200 | 0x0800_0000,
            "upstream stdio MCPs must not create a visible console window"
        );
    }

    #[tokio::test]
    async fn stdio_manager_discovers_defaults_and_honors_disabled_tool() {
        let Ok(node) = which::which("node") else {
            // Node is a desktop-app prerequisite. Keep Rust-only environments
            // able to run the unit suite without treating this integration
            // fixture as a failed product behavior.
            return;
        };
        let directory = tempfile::tempdir().expect("fixture directory");
        let fixture = directory.path().join("fixture-mcp.cjs");
        fs::write(
            &fixture,
            r#"
const readline = require('node:readline');
const output = (message) => process.stdout.write(JSON.stringify(message) + '\n');
readline.createInterface({ input: process.stdin }).on('line', (line) => {
  const request = JSON.parse(line);
  if (request.method === 'initialize') {
    output({ jsonrpc: '2.0', id: request.id, result: { protocolVersion: '2025-06-18', capabilities: {}, serverInfo: { name: 'fixture', version: '1.0.0' } } });
  } else if (request.method === 'tools/list') {
    output({ jsonrpc: '2.0', id: request.id, result: { tools: [
      { name: 'status', description: 'fixture status', inputSchema: { type: 'object', properties: {}, additionalProperties: false } },
      { name: 'hidden', description: 'fixture hidden', inputSchema: { type: 'object', properties: {}, additionalProperties: false } }
    ] } });
  } else if (request.method === 'tools/call') {
    output({ jsonrpc: '2.0', id: request.id, result: { content: [{ type: 'text', text: request.params.name }] } });
  }
});
"#,
        )
        .expect("write fixture");
        let config = UpstreamMcpConfig {
            id: "fixture".to_string(),
            name: "Fixture".to_string(),
            enabled: true,
            transport: "stdio".to_string(),
            command: node.to_string_lossy().to_string(),
            args: vec![fixture.to_string_lossy().to_string()],
            env: BTreeMap::new(),
            url: String::new(),
            headers: BTreeMap::new(),
            tool_prefix: "fixture".to_string(),
            allowed_tools: Vec::new(),
            disabled_tools: vec!["hidden".to_string()],
        };

        let inspected = UpstreamMcpManager::inspect(&config)
            .await
            .expect("inspect fixture");
        assert_eq!(inspected.len(), 2);
        assert_eq!(inspected[0].name, "status");
        assert_eq!(inspected[0].input_schema["type"], "object");

        let manager = UpstreamMcpManager::start(&[config], &["server_info"])
            .await
            .expect("start fixture");
        assert_eq!(manager.public_tools().len(), 1);
        assert_eq!(manager.public_tools()[0]["name"], "fixture__status");
        assert!(!manager.owns_tool("fixture__hidden"));
        let result = manager
            .call_tool("fixture__status", json!({}))
            .await
            .expect("forward fixture call");
        assert_eq!(result["content"][0]["text"], "status");
        manager.shutdown().await;
    }

    #[derive(Clone)]
    struct StreamableFixtureState {
        session_seen: Arc<AtomicBool>,
        custom_header_seen: Arc<AtomicBool>,
    }

    async fn streamable_http_fixture(
        State(state): State<StreamableFixtureState>,
        headers: HeaderMap,
        Json(request): Json<Value>,
    ) -> (HeaderMap, String) {
        let id = request["id"].clone();
        let method = request["method"].as_str().unwrap_or_default();
        let mut response_headers = HeaderMap::new();
        response_headers.insert("mcp-session-id", HeaderValue::from_static("fixture-session"));

        if method != "initialize" {
            state.session_seen.store(
                headers
                    .get("mcp-session-id")
                    .is_some_and(|value| value == "fixture-session"),
                Ordering::SeqCst,
            );
        }
        state.custom_header_seen.store(
            headers
                .get("x-fixture")
                .is_some_and(|value| value == "true"),
            Ordering::SeqCst,
        );

        let response = match method {
            "initialize" => json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": { "protocolVersion": "2025-06-18", "capabilities": {}, "serverInfo": { "name": "fixture", "version": "1.0.0" } }
            }),
            "tools/list" => {
                response_headers.insert(CONTENT_TYPE, HeaderValue::from_static("text/event-stream"));
                json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": { "tools": [
                        { "name": "status", "description": "http fixture status", "inputSchema": { "type": "object", "properties": {}, "additionalProperties": false } },
                        { "name": "hidden", "description": "http fixture hidden", "inputSchema": { "type": "object", "properties": {}, "additionalProperties": false } }
                    ] }
                })
            }
            "tools/call" => json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": { "content": [{ "type": "text", "text": request["params"]["name"] }] }
            }),
            _ => json!({ "jsonrpc": "2.0", "id": id, "result": {} }),
        };
        let body = if method == "tools/list" {
            format!("event: message\ndata: {response}\n\n")
        } else {
            response.to_string()
        };
        (response_headers, body)
    }

    #[tokio::test]
    async fn streamable_http_manager_discovers_forwards_and_reuses_session() {
        let state = StreamableFixtureState {
            session_seen: Arc::new(AtomicBool::new(false)),
            custom_header_seen: Arc::new(AtomicBool::new(false)),
        };
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind fixture");
        let address = listener.local_addr().expect("fixture address");
        let app = Router::new()
            .route("/mcp", post(streamable_http_fixture))
            .with_state(state.clone());
        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.expect("serve fixture");
        });

        let config = UpstreamMcpConfig {
            id: "http-fixture".to_string(),
            name: "HTTP Fixture".to_string(),
            enabled: true,
            transport: "streamableHttp".to_string(),
            command: String::new(),
            args: Vec::new(),
            env: BTreeMap::new(),
            url: format!("http://{address}/mcp"),
            headers: BTreeMap::from([("X-Fixture".to_string(), "true".to_string())]),
            tool_prefix: "http".to_string(),
            allowed_tools: Vec::new(),
            disabled_tools: vec!["hidden".to_string()],
        };

        let manager = UpstreamMcpManager::start(&[config], &[])
            .await
            .expect("start HTTP fixture");
        assert_eq!(manager.public_tools().len(), 1);
        assert_eq!(manager.public_tools()[0]["name"], "http__status");
        let result = manager
            .call_tool("http__status", json!({}))
            .await
            .expect("forward HTTP fixture call");
        assert_eq!(result["content"][0]["text"], "status");
        assert!(state.session_seen.load(Ordering::SeqCst));
        assert!(state.custom_header_seen.load(Ordering::SeqCst));
        manager.shutdown().await;
        server.abort();
    }
}
