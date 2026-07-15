mod common;

use std::fs;
use std::process::Command;

use common::*;
use serde_json::{json, Value};

#[test]
fn server_info_returns_workspace_and_tools() {
    let fx = tiny_js_fixture();
    let ctx = ctx_for(&fx.root);
    let out = invoke(&ctx, "server_info", json!({}));
    let payload = assert_ok(&out);
    assert_eq!(payload["server"], "coding-tools-mcp");
    assert_eq!(payload["version"], env!("CARGO_PKG_VERSION"));
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
fn read_file_explicit_parent_path_is_read_only() {
    let fx = malicious_fixture();
    let ctx = ctx_for(&fx.root);
    let out = invoke(&ctx, "read_file", json!({"path": "../outside-secret.txt"}));
    let result = assert_ok(&out);
    assert!(result["content"].as_str().unwrap_or("").contains("TOP_SECRET"));
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
fn default_cwd_is_used_by_file_and_native_exec_tools() {
    let fx = tiny_js_fixture();
    let ctx = ctx_for(&fx.root);
    assert_ok(&invoke(&ctx, "set_default_cwd", json!({"path": "src"})));

    let file_result = invoke(&ctx, "read_file", json!({"path": "math.js"}));
    let file = assert_ok(&file_result);
    assert_eq!(file["path"], "src/math.js");

    let pwd_result = invoke(&ctx, "exec_command", json!({"cmd": "pwd"}));
    let pwd = assert_ok(&pwd_result);
    assert!(pwd["stdout"].as_str().unwrap_or("").contains("src"));
}

#[test]
fn git_log_root_does_not_pass_empty_pathspec() {
    let temp = tempfile::tempdir().expect("创建临时目录");
    let workspace = temp.path().join("repo");
    fs::create_dir_all(&workspace).expect("创建仓库目录");
    fs::write(workspace.join("README.md"), "初始内容\n").expect("写入文件");

    for args in [
        vec!["init", "-q"],
        vec!["config", "user.email", "test@example.com"],
        vec!["config", "user.name", "测试用户"],
        vec!["add", "README.md"],
        vec!["commit", "-q", "-m", "初始化"],
    ] {
        let output = Command::new("git")
            .current_dir(&workspace)
            .args(args)
            .output()
            .expect("执行 git");
        assert!(output.status.success(), "git 命令失败: {:?}", output);
    }

    let ctx = ctx_for(&workspace);
    let result = invoke(
        &ctx,
        "git_log",
        json!({"path": ".", "max_count": 3}),
    );
    let payload = assert_ok(&result);
    assert_eq!(payload["is_repo"], true);
    assert_eq!(payload["commits"].as_array().unwrap().len(), 1);
    for commit in payload["commits"].as_array().unwrap() {
        for field in ["hash", "short_hash", "author_name", "author_email", "author_date", "subject"] {
            assert_eq!(commit[field].as_str().unwrap(), commit[field].as_str().unwrap().trim());
        }
    }
}

#[test]
fn advanced_profile_exposes_every_declared_tool() {
    let declared = coding_tools_mcp_desktop_lib::tools::registry::P0_TOOLS
        .iter()
        .map(|(name, ..)| *name)
        .collect::<std::collections::HashSet<_>>();
    let tool_values = coding_tools_mcp_desktop_lib::tools::list_tools_for_profile("advanced");
    let exposed = tool_values
        .iter()
        .filter_map(|tool| tool["name"].as_str())
        .collect::<std::collections::HashSet<_>>();

    assert_eq!(declared, exposed);
    assert!(declared
        .iter()
        .all(|name| coding_tools_mcp_desktop_lib::tools::is_allowed_tool(name)));
}

#[test]
fn core_profile_matches_the_old_default_toolset() {
    let tools = coding_tools_mcp_desktop_lib::tools::list_tools_for_profile("core");
    let names = tools
        .iter()
        .filter_map(|tool| tool["name"].as_str())
        .collect::<std::collections::HashSet<_>>();
    let expected = coding_tools_mcp_desktop_lib::tools::registry::CORE_TOOLS
        .iter()
        .copied()
        .collect::<std::collections::HashSet<_>>();
    assert_eq!(names, expected);
    assert_eq!(names.len(), 21);
    assert!(names.contains("grep"));
    assert!(!names.contains("harness_status"));
    assert!(!names.contains("start_task"));
}

#[test]
fn exec_health_check_reports_worker_and_pipe_status() {
    let fx = tiny_js_fixture();
    let ctx = ctx_for(&fx.root);
    let out = invoke(&ctx, "exec_health_check", json!({}));
    let payload = assert_ok(&out);
    assert_eq!(payload["worker"]["alive"], true);
    assert_eq!(payload["session_create"], true);
    assert_eq!(payload["command_run"], true);
    assert_eq!(payload["stdout_capture"], true);
    assert_eq!(payload["stderr_capture"], true);
}

#[test]
fn native_diagnostics_support_pwd_and_ls_without_a_shell() {
    let fx = tiny_js_fixture();
    let ctx = ctx_for(&fx.root);

    let pwd_result = invoke(&ctx, "exec_command", json!({"cmd": "pwd"}));
    let pwd = assert_ok(&pwd_result);
    assert_eq!(pwd["command"], "pwd");
    assert!(pwd["stdout"]
        .as_str()
        .unwrap_or("")
        .contains("tiny-js-project"));
    assert_eq!(pwd["execution_mode"], "native_builtin");
    assert_eq!(pwd["harness_mode"], "standalone");
    assert_eq!(pwd["task_required"], false);
    assert_eq!(pwd["command_runner"], "native_builtin");
    assert_eq!(pwd["status"], "exited");
    assert_eq!(pwd["exit_code"], 0);
    assert_eq!(pwd["transport_ok"], true);
    assert_eq!(pwd["command_ok"], true);
    assert_eq!(pwd["duration_ms"], 0);
    assert_eq!(pwd["elapsed_ms"], 0);
    assert!(pwd["stdout"].is_string());
    assert_eq!(pwd["stderr"], "");

    let ls_result = invoke(&ctx, "exec_command", json!({"cmd": "ls"}));
    let ls = assert_ok(&ls_result);
    assert!(ls["stdout"].as_str().unwrap_or("").contains("src"));
    assert_eq!(ls["exit_code"], 0);
}

#[test]
fn direct_exec_uses_the_same_result_contract() {
    let fx = tiny_js_fixture();
    let ctx = ctx_for(&fx.root);
    let result = invoke(
        &ctx,
        "exec_command",
        json!({"cmd": "python --version", "filesystem_scope": "workspace"}),
    );
    let payload = assert_ok(&result);

    assert_eq!(payload["command"], "python --version");
    assert_eq!(payload["execution_mode"], "direct");
    assert_eq!(payload["harness_mode"], "standalone");
    assert_eq!(payload["task_required"], false);
    assert_eq!(payload["status"], "exited");
    assert_eq!(payload["exit_code"], 0);
    assert!(payload["stdout"].is_string());
    assert!(payload["stderr"].is_string());
    assert!(payload["duration_ms"].is_u64());
    assert_eq!(payload["duration_ms"], payload["elapsed_ms"]);
    assert_eq!(payload["transport_ok"], true);
    assert_eq!(payload["command_ok"], true);
}

#[test]
fn nonzero_command_exit_keeps_transport_ok_but_sets_command_ok_false() {
    let fx = tiny_js_fixture();
    let ctx = ctx_for(&fx.root);
    let result = invoke(
        &ctx,
        "exec_command",
        json!({
            "cmd": "python -c \"import sys; sys.exit(1)\"",
            "filesystem_scope": "workspace"
        }),
    );
    let payload = assert_ok(&result);

    assert_eq!(payload["ok"], true);
    assert_eq!(payload["transport_ok"], true);
    assert_eq!(payload["command_ok"], false);
    assert_eq!(payload["status"], "exited");
    assert_eq!(payload["exit_code"], 1);
}

#[test]
fn retained_session_timeout_stops_the_process_after_deadline() {
    let fx = tiny_js_fixture();
    let ctx = ctx_for(&fx.root);
    let result = invoke(
        &ctx,
        "exec_command",
        json!({
            "cmd": "python -c \"import time; time.sleep(2)\"",
            "filesystem_scope": "workspace",
            "timeout_ms": 100,
            "yield_time_ms": 0
        }),
    );
    let payload = assert_ok(&result);
    assert_eq!(payload["status"], "running");
    assert_eq!(payload["transport_ok"], true);
    assert_eq!(payload["command_ok"], Value::Null);
    assert_eq!(payload["stdin_open"], true);
    let session_id = payload["session_id"].as_str().expect("session id");

    std::thread::sleep(std::time::Duration::from_millis(250));
    let after = invoke(
        &ctx,
        "write_stdin",
        json!({"session_id": session_id, "chars": ""}),
    );
    assert_eq!(after["termination_reason"], "timeout");
    assert_eq!(after["status"], "exited");
    assert_eq!(after["transport_ok"], true);
    assert_eq!(after["command_ok"], false);
    assert_eq!(after["stdin_open"], false);
}

#[test]
fn killed_session_reports_command_failure_even_when_transport_succeeds() {
    let fx = tiny_js_fixture();
    let ctx = ctx_for(&fx.root);
    let result = invoke(
        &ctx,
        "exec_command",
        json!({
            "cmd": "python -c \"import time; time.sleep(2)\"",
            "filesystem_scope": "workspace",
            "timeout_ms": 10_000,
            "yield_time_ms": 0
        }),
    );
    let payload = assert_ok(&result);
    let session_id = payload["session_id"].as_str().expect("session id");

    let killed = invoke(
        &ctx,
        "kill_session",
        json!({"session_id": session_id, "wait_ms": 2_000}),
    );
    let killed = assert_ok(&killed);
    assert_eq!(killed["status"], "killed");
    assert_eq!(killed["killed"], true);
    assert_eq!(killed["transport_ok"], true);
    assert_eq!(killed["command_ok"], false);
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

#[test]
fn grep_reuses_search_text_schema_and_behavior() {
    let schema = coding_tools_mcp_desktop_lib::tools::registry::input_schema("grep");
    assert_eq!(
        schema,
        coding_tools_mcp_desktop_lib::tools::registry::input_schema("search_text")
    );

    let fx = tiny_js_fixture();
    let ctx = ctx_for(&fx.root);
    assert_ok(&invoke(&ctx, "set_default_cwd", json!({"path": "src"})));
    let output = invoke(
        &ctx,
        "grep",
        json!({
            "query": "function\\s+add",
            "path": ".",
            "glob": "**/*.js",
            "regex": true,
            "case_sensitive": true,
            "max_results": 10
        }),
    );
    let payload = assert_ok(&output);
    let matches = payload["matches"].as_array().expect("matches array");
    assert!(!matches.is_empty());
    assert!(matches
        .iter()
        .all(|item| item["path"].as_str().unwrap_or("").starts_with("src/")));
}
