mod common;

use common::*;
use serde_json::json;

#[test]
fn server_info_returns_workspace_and_tools() {
    let fx = tiny_js_fixture();
    let ctx = ctx_for(&fx.root);
    let out = invoke(&ctx, "server_info", json!({}));
    let payload = assert_ok(&out);
    assert_eq!(payload["server"], "coding-tools-mcp");
    assert!(payload["tools"].is_array());
    assert!(payload["tool_count"].as_u64().unwrap_or(0) > 0);
}

#[test]
fn read_file_happy_path() {
    let fx = tiny_js_fixture();
    let ctx = ctx_for(&fx.root);
    let out = invoke(&ctx, "read_file", json!({"path": "src/math.js"}));
    let payload = assert_ok(&out);
    assert_eq!(payload["path"], "src/math.js");
    assert_eq!(payload["encoding"], "utf-8");
}

#[test]
fn unknown_tool_is_validation_error() {
    let fx = tiny_js_fixture();
    let ctx = ctx_for(&fx.root);
    let out = invoke(&ctx, "definitely_not_a_tool", json!({}));
    let err = assert_err(&out);
    assert_eq!(err["error"]["code"], "INVALID_ARGUMENT");
    assert_eq!(err["error"]["category"], "validation");
}

#[test]
fn read_file_traversal_has_structured_security_error() {
    let fx = malicious_fixture();
    let ctx = ctx_for(&fx.root);
    let out = invoke(&ctx, "read_file", json!({"path": "../outside-secret.txt"}));
    assert_security_or_policy_err(&out);
}

#[test]
fn request_permissions_is_unsupported_not_silent_grant() {
    let fx = tiny_js_fixture();
    let ctx = ctx_for(&fx.root);
    let out = invoke(
        &ctx,
        "request_permissions",
        json!({
            "tool_name": "exec_command",
            "permission": "network",
            "reason": "verify compliance denial shape",
            "arguments": {"cmd": "curl https://example.com"}
        }),
    );
    assert_err(&out);
    assert_eq!(out["error"]["code"], "ELICITATION_UNSUPPORTED");
    assert_eq!(out["status"], "unsupported");
}

#[test]
fn check_exec_environment_reports_policy_metadata() {
    let fx = tiny_js_fixture();
    let ctx = ctx_for(&fx.root);
    let out = invoke(&ctx, "check_exec_environment", json!({}));
    let payload = assert_ok(&out);
    assert_eq!(payload["permission_mode"], "trusted");
    assert!(payload["allowed_commands"].is_array());
}

#[test]
fn list_files_accepts_glob_alias() {
    let fx = tiny_js_fixture();
    let ctx = ctx_for(&fx.root);
    let out = invoke(
        &ctx,
        "list_files",
        json!({"glob": "**/*.js", "max_results": 10}),
    );
    let payload = assert_ok(&out);
    let files = payload["files"].as_array().expect("files array");
    assert!(!files.is_empty());
    assert!(
        files
            .iter()
            .all(|f| f["path"].as_str().unwrap_or("").ends_with(".js"))
    );
}

#[test]
fn search_text_filters_by_glob() {
    let fx = tiny_js_fixture();
    let ctx = ctx_for(&fx.root);
    let hit = invoke(
        &ctx,
        "search_text",
        json!({"query": "function add", "glob": "**/*.js", "max_results": 10}),
    );
    let hit_payload = assert_ok(&hit);
    assert!(hit_payload["total_matches"].as_u64().unwrap_or(0) > 0);

    let miss = invoke(
        &ctx,
        "search_text",
        json!({"query": "function add", "glob": "**/*.py"}),
    );
    let miss_payload = assert_ok(&miss);
    assert_eq!(miss_payload["total_matches"].as_u64().unwrap_or(1), 0);
}
