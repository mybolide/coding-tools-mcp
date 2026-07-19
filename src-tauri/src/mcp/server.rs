use std::sync::Arc;

use serde_json::Value;

use crate::tools::{
    call_tool, list_tools_for_profile, wrap_mcp_tool_result, SharedToolContext, ToolContext,
    Workspace,
};
use crate::workspace::AuthConfig;

pub type SharedState = SharedToolContext;

pub fn handle_request(state: &SharedState, body: &Value) -> Value {
    let method = body.get("method").and_then(Value::as_str).unwrap_or("");
    let id = body.get("id").cloned().unwrap_or(Value::Null);
    let params = body.get("params").cloned().unwrap_or(Value::Null);

    if id.is_null() && method.starts_with("notifications/") {
        return Value::Null;
    }

    let result = match method {
        "initialize" => Ok(initialize_result()),
        "ping" => Ok(serde_json::json!({})),
        "tools/list" => {
            let tools = list_tools_for_profile(&state.tool_profile);
            Ok(serde_json::json!({ "tools": tools }))
        }
        "tools/call" => handle_tools_call(state, &params),
        _ => Err(serde_json::json!({
            "code": -32601,
            "message": format!("Method not found: {method}")
        })),
    };

    match result {
        Ok(result) => serde_json::json!({ "jsonrpc": "2.0", "id": id, "result": result }),
        Err(error) => serde_json::json!({ "jsonrpc": "2.0", "id": id, "error": error }),
    }
}

fn initialize_result() -> Value {
    serde_json::json!({
        "protocolVersion": "2025-06-18",
        "capabilities": {
            "tools": { "listChanged": false },
            "logging": {}
        },
        "serverInfo": {
            "name": "coding-tools-mcp",
            "title": "Coding Tools MCP",
            "version": env!("CARGO_PKG_VERSION")
        },
        "instructions": "Use these tools only for local coding operations inside the configured workspace. At the start of every new ChatGPT conversation, before answering the user's first request, call history_session_bootstrap exactly once, even if the user did not explicitly ask to restore or resume. Treat bootstrap as required conversation initialization: when no history exists it creates the first history session; when history exists, read all_history_summary, latest_handoff, and inherited_summary before acting. Repeated successful bootstrap calls in the same conversation resume the same session and must not create duplicates. Preserve session_key and current_path returned by bootstrap, then pass them unchanged as session_key and expected_path to every history_session_checkpoint call. After completing each user-requested task in the conversation, call history_session_checkpoint before the final response. Only state that progress was saved after checkpoint returns ok=true with the same session_key and path. Persistence requires a successful tool call and is not automatic background persistence."
    })
}

fn handle_tools_call(state: &SharedState, params: &Value) -> Result<Value, Value> {
    let name = params
        .get("name")
        .and_then(Value::as_str)
        .ok_or_else(|| serde_json::json!({ "code": -32602, "message": "Missing tool name" }))?;
    let args = tool_arguments(name, params);

    let canonical_name = crate::tools::registry::canonical_tool_name(name);
    let known = crate::tools::registry::exposed_tool_names(&state.tool_profile);
    if !known.iter().any(|n| n == &canonical_name) {
        return Err(serde_json::json!({
            "code": -32602,
            "message": format!("Unknown tool: {name}"),
            "data": { "reason": "unknown_tool" }
        }));
    }

    let structured = call_tool(state.as_ref(), canonical_name, &args);
    Ok(wrap_mcp_tool_result(canonical_name, &args, structured))
}

fn tool_arguments(name: &str, params: &Value) -> Value {
    let mut args = params
        .get("arguments")
        .cloned()
        .unwrap_or_else(|| serde_json::json!({}));
    if name.starts_with("history_session_") {
        if let Some(session_key) = params
            .get("_meta")
            .and_then(|meta| meta.get("openai/session"))
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            if !args.is_object() {
                args = serde_json::json!({});
            }
            args["_host_session_key"] = Value::String(session_key.to_string());
        }
    }
    args
}

pub fn new_state(
    workspace: Workspace,
    auth: AuthConfig,
    policy: crate::tools::policy::PolicySettings,
    tool_profile: String,
    permission_mode: String,
) -> SharedState {
    Arc::new(ToolContext::from_workspace(
        workspace,
        auth,
        policy,
        tool_profile,
        permission_mode,
    ))
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::sync::Arc;

    use serde_json::json;

    use crate::tools::ToolContext;

    use super::{handle_request, initialize_result, tool_arguments};

    #[test]
    fn initialize_instructions_define_the_history_persistence_workflow() {
        let initialized = initialize_result();
        let instructions = initialized["instructions"].as_str().expect("instructions");
        assert!(instructions.contains("history_session_bootstrap"));
        assert!(instructions.contains("At the start of every new ChatGPT conversation"));
        assert!(instructions.contains("before answering the user's first request"));
        assert!(instructions.contains("even if the user did not explicitly ask"));
        assert!(instructions.contains("required conversation initialization"));
        assert!(instructions.contains("must not create duplicates"));
        assert!(instructions.contains("history_session_checkpoint"));
        assert!(instructions.contains("session_key and current_path returned by bootstrap"));
        assert!(instructions.contains("session_key and expected_path"));
        assert!(instructions.contains("After completing each user-requested task"));
        assert!(instructions.contains("before the final response"));
        assert!(instructions.contains("checkpoint returns ok=true"));
        assert!(instructions.contains("not automatic background persistence"));
    }

    #[test]
    fn initialize_does_not_claim_tool_catalog_notifications_without_a_stream() {
        let initialized = initialize_result();

        assert_eq!(initialized["capabilities"]["tools"]["listChanged"], false);
    }

    #[test]
    fn workspace_prompt_initializes_or_restores_a_chatgpt_session() {
        let component = include_str!("../../../src/lib/components/ChatGptSessionPrompt.svelte");

        assert!(component.contains("ChatGPT 新会话启动提示词"));
        assert!(component.contains("请初始化或恢复当前项目会话"));
        assert!(component.contains("如果没有历史记录"));
        assert!(component.contains("all_history_summary"));
        assert!(component.contains("history_session_checkpoint"));
        assert!(!component.contains("打开连接器设置"));
    }

    #[test]
    fn chatgpt_session_metadata_is_injected_only_for_history_tools() {
        let params = json!({
            "arguments": {"session_key": "explicit"},
            "_meta": {"openai/session": "chatgpt-conversation"}
        });
        let history = tool_arguments("history_session_bootstrap", &params);
        assert_eq!(history["session_key"], "explicit");
        assert_eq!(history["_host_session_key"], "chatgpt-conversation");

        let existing = tool_arguments("read_file", &params);
        assert_eq!(existing["session_key"], "explicit");
        assert!(existing.get("_host_session_key").is_none());
    }

    #[test]
    fn explicit_session_key_prevents_changed_chatgpt_metadata_from_redirecting_history() {
        let workspace = tempfile::tempdir().expect("workspace tempdir");
        let harness = tempfile::tempdir().expect("harness tempdir");
        let state = Arc::new(
            ToolContext::for_test(workspace.path().to_path_buf(), harness.path().to_path_buf())
                .expect("tool context"),
        );
        let response = handle_request(
            &state,
            &json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "tools/call",
                "params": {
                    "name": "history_session_bootstrap",
                    "arguments": {"session_key": "explicit-session"},
                    "_meta": {"openai/session": "chatgpt-session"}
                }
            }),
        );
        let structured = &response["result"]["structuredContent"];
        assert_eq!(structured["ok"], true);
        assert_eq!(structured["session_key_source"], "explicit_session_key");
        assert_eq!(structured["session_key"], "explicit-session");
        assert_eq!(structured["host_session_key_mismatch"], true);
        let content = fs::read_to_string(workspace.path().join("docs/history-session/1.md"))
            .expect("read history file");
        assert!(content.contains("**Session key:** explicit-session"));
        assert!(!content.contains("**Session key:** chatgpt-session"));
    }

    #[test]
    fn legacy_grep_calls_are_mapped_to_the_public_grep_text_tool() {
        let workspace = tempfile::tempdir().expect("workspace tempdir");
        let harness = tempfile::tempdir().expect("harness tempdir");
        fs::write(workspace.path().join("sample.txt"), "catalog needle")
            .expect("write sample file");
        let state = Arc::new(
            ToolContext::for_test(workspace.path().to_path_buf(), harness.path().to_path_buf())
                .expect("tool context"),
        );

        let response = handle_request(
            &state,
            &json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "tools/call",
                "params": {
                    "name": "grep",
                    "arguments": {"query": "needle", "path": "."}
                }
            }),
        );

        assert!(response.get("error").is_none());
        assert_eq!(response["result"]["structuredContent"]["ok"], true);
    }
}
