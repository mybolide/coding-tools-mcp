mod common;

use common::*;
use serde_json::json;

const TRAVERSAL_PATCH: &str = r#"*** Begin Patch
*** Update File: ../outside-secret.txt
@@
-TOP_SECRET_DO_NOT_READ
+unsafe
*** End Patch
"#;

#[test]
fn read_file_rejects_symlink_escape() {
    let fx = malicious_fixture();
    let ctx = ctx_for(&fx.root);
    let out = invoke(&ctx, "read_file", json!({"path": "outside-link.txt"}));
    if fx.root.join("outside-link.txt").exists() {
        assert_security_or_policy_err(&out);
    }
}

#[test]
fn apply_patch_rejects_traversal_target() {
    let fx = malicious_fixture();
    let ctx = ctx_for(&fx.root);
    let out = invoke(&ctx, "apply_patch", json!({"patch": TRAVERSAL_PATCH}));
    assert_security_or_policy_err(&out);
}

#[test]
fn exec_command_rejects_workdir_escape_via_policy() {
    assert_policy_rejects("exec_command", json!({"cmd": "pwd", "workdir": ".."}));
}

#[test]
fn exec_command_rejects_disallowed_destructive_command() {
    assert_policy_rejects("exec_command", json!({"cmd": "rm -rf /"}));
}

#[test]
fn exec_command_rejects_python_c_interpreter_escape() {
    assert_policy_rejects(
        "exec_command",
        json!({
            "cmd": "python -c \"import urllib.request; urllib.request.urlopen('https://example.com', timeout=1)\"",
            "timeout_ms": 3000
        }),
    );
}

#[test]
fn exec_command_rejects_shell_chaining() {
    assert_policy_rejects("exec_command", json!({"cmd": "echo hi && rm -rf /"}));
}

#[test]
fn safe_permission_mode_blocks_network_looking_command() {
    let mut policy = coding_tools_mcp_desktop_lib::tools::policy::PolicySettings::default();
    policy.permission_mode = "safe".into();
    let err = coding_tools_mcp_desktop_lib::tools::policy::validate_tool_arguments(
        "exec_command",
        &json!({"cmd": "curl https://example.com"}),
        &policy,
    )
    .expect_err("network command should be blocked in safe mode");
    assert!(err.0.contains("Network-looking"));
}
