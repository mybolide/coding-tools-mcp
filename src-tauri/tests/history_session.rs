mod common;

use std::fs;
use std::sync::{Arc, Barrier};

use coding_tools_mcp_desktop_lib::tools::{list_tools_for_profile, ToolContext};
use serde_json::{json, Value};

use common::{assert_err, assert_ok, invoke};

fn test_context() -> (tempfile::TempDir, tempfile::TempDir, ToolContext) {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let harness = tempfile::tempdir().expect("harness tempdir");
    let ctx = ToolContext::for_test(workspace.path().to_path_buf(), harness.path().to_path_buf())
        .expect("tool context");
    (workspace, harness, ctx)
}

#[test]
fn checkpoint_keeps_bootstrap_target_when_host_session_metadata_changes() {
    let (workspace, _harness, ctx) = test_context();
    let boot = invoke(
        &ctx,
        "history_session_bootstrap",
        json!({
            "session_key": "stable-bootstrap-key",
            "_host_session_key": "host-before"
        }),
    );
    let boot = assert_ok(&boot);
    assert_eq!(boot["session_key"], "stable-bootstrap-key");
    assert_eq!(boot["current_path"], "docs/history-session/1.md");
    assert_eq!(boot["host_session_key_mismatch"], true);

    let checkpoint = invoke(
        &ctx,
        "history_session_checkpoint",
        json!({
            "session_key": boot["session_key"],
            "expected_path": boot["current_path"],
            "_host_session_key": "host-after",
            "turn_id": "stable-turn",
            "user_intent": "不能串写"
        }),
    );
    let checkpoint = assert_ok(&checkpoint);
    assert_eq!(checkpoint["path"], boot["current_path"]);
    assert_eq!(checkpoint["session_key"], boot["session_key"]);
    assert_eq!(checkpoint["host_session_key_mismatch"], true);
    assert!(!workspace.path().join("docs/history-session/2.md").exists());
}

#[test]
fn checkpoint_rejects_a_path_from_another_session() {
    let (_workspace, _harness, ctx) = test_context();
    assert_ok(&invoke(
        &ctx,
        "history_session_bootstrap",
        json!({"session_key": "session-a"}),
    ));
    assert_ok(&invoke(
        &ctx,
        "history_session_bootstrap",
        json!({"session_key": "session-b"}),
    ));

    let result = invoke(
        &ctx,
        "history_session_checkpoint",
        json!({
            "session_key": "session-a",
            "expected_path": "docs/history-session/2.md",
            "turn_id": "wrong-target"
        }),
    );
    assert_eq!(
        assert_err(&result)["error"]["code"],
        "SESSION_TARGET_MISMATCH"
    );
}

#[test]
fn inherited_summary_is_preserved_without_recursive_growth() {
    let (workspace, _harness, ctx) = test_context();
    prepare_history(workspace.path());
    let boot = invoke(
        &ctx,
        "history_session_bootstrap",
        json!({"session_key": "summary-session"}),
    );
    let boot = assert_ok(&boot);
    assert_ok(&invoke(
        &ctx,
        "history_session_checkpoint",
        json!({
            "session_key": boot["session_key"],
            "expected_path": boot["current_path"],
            "turn_id": "summary-turn",
            "user_intent": "继续实现"
        }),
    ));
    let content = fs::read_to_string(workspace.path().join("docs/history-session/3.md"))
        .expect("read preserved inherited summary");
    assert_eq!(content.matches("## 继承的历史摘要").count(), 1);
    assert!(content.contains("目标-第一阶段"));
    assert!(content.contains("继续实现"));

    let next = invoke(
        &ctx,
        "history_session_bootstrap",
        json!({"session_key": "next-summary-session"}),
    );
    assert_ok(&next);
    let next_content = fs::read_to_string(workspace.path().join("docs/history-session/4.md"))
        .expect("read next inherited summary");
    assert_eq!(next_content.matches("## 继承的历史摘要").count(), 1);
    assert!(next_content.contains("### 会话 3（docs/history-session/3.md）"));
}

#[test]
fn inherited_summary_is_bounded_and_reports_omitted_sessions() {
    let (workspace, _harness, ctx) = test_context();
    let dir = workspace.path().join("docs/history-session");
    fs::create_dir_all(&dir).expect("create history dir");
    let large_marker = "X".repeat(4_000);
    for number in 1..=12 {
        fs::write(
            dir.join(format!("{number}.md")),
            history_file(number, &format!("session-{number}"), &large_marker),
        )
        .expect("write large history");
    }
    let boot = invoke(
        &ctx,
        "history_session_bootstrap",
        json!({"session_key": "bounded-summary"}),
    );
    assert_ok(&boot);
    let content = fs::read_to_string(dir.join("13.md")).expect("read bounded summary");
    assert!(content.contains("个较早会话未展开"));
    assert!(content.chars().count() < 20_000);
}

fn history_file(number: u64, session_key: &str, marker: &str) -> String {
    format!(
        "# 会话 {number}：{marker}\n\n\
**Session key:** {session_key}\n\
**Created:** 2026-07-17T08:00:00+08:00\n\
**Updated:** 2026-07-17T09:00:00+08:00\n\
**Status:** completed\n\n\
## 用户核心目标\n\n目标-{marker}\n\n\
## 已确认事实\n\n事实-{marker}\n\n\
## 已完成修改\n\n修改-{marker}\n\n\
## 关键设计决定\n\n决定-{marker}\n\n\
## 测试结果\n\n测试-{marker}\n\n\
## 当前运行状态\n\n运行-{marker}\n\n\
## 剩余问题\n\n问题-{marker}\n\n\
## 下一步\n\n下一步-{marker}\n\n\
## 本轮检查点\n"
    )
}

fn prepare_history(root: &std::path::Path) {
    let dir = root.join("docs/history-session");
    fs::create_dir_all(&dir).expect("create history dir");
    fs::write(dir.join("README.md"), "# 历史归档说明\n").expect("write readme");
    fs::write(
        dir.join("1.md"),
        history_file(1, "old-session-1", "第一阶段"),
    )
    .expect("write 1.md");
    fs::write(
        dir.join("2.md"),
        history_file(2, "old-session-2", "第二阶段"),
    )
    .expect("write 2.md");
}

#[test]
fn history_tools_are_exposed_with_public_schemas() {
    let tools = list_tools_for_profile("core");
    for name in [
        "history_session_bootstrap",
        "history_session_checkpoint",
        "history_session_validate",
    ] {
        let tool = tools
            .iter()
            .find(|tool| tool["name"] == name)
            .unwrap_or_else(|| panic!("missing tool: {name}"));
        assert_eq!(tool["inputSchema"]["type"], "object");
        assert_eq!(tool["inputSchema"]["additionalProperties"], false);
        assert!(tool["inputSchema"]["properties"]
            .get("_host_session_key")
            .is_none());
    }

    let bootstrap = tools
        .iter()
        .find(|tool| tool["name"] == "history_session_bootstrap")
        .expect("bootstrap descriptor");
    assert!(bootstrap["description"]
        .as_str()
        .unwrap_or("")
        .contains("restore"));
    let checkpoint_description = tools
        .iter()
        .find(|tool| tool["name"] == "history_session_checkpoint")
        .expect("checkpoint descriptor")["description"]
        .as_str()
        .unwrap_or("");
    assert!(!checkpoint_description.contains("before every final response"));
    assert!(!checkpoint_description.contains("ChatGPT"));

    let checkpoint = tools
        .iter()
        .find(|tool| tool["name"] == "history_session_checkpoint")
        .expect("checkpoint schema");
    assert_eq!(
        checkpoint["inputSchema"]["required"],
        json!(["session_key", "expected_path"])
    );
}

#[test]
fn bootstrap_requires_a_stable_session_id() {
    let (_workspace, _harness, ctx) = test_context();
    let result = invoke(&ctx, "history_session_bootstrap", json!({}));
    let payload = assert_err(&result);
    assert_eq!(payload["error"]["code"], "SESSION_ID_UNAVAILABLE");
}

#[test]
fn workspace_root_accepts_dot_and_current_absolute_path_but_rejects_outside() {
    let (workspace, _harness, ctx) = test_context();
    let relative = invoke(
        &ctx,
        "history_session_bootstrap",
        json!({"workspace_root": ".", "session_key": "relative-root"}),
    );
    assert_eq!(assert_ok(&relative)["current_number"], 1);

    let absolute = invoke(
        &ctx,
        "history_session_bootstrap",
        json!({
            "workspace_root": workspace.path().to_string_lossy(),
            "session_key": "absolute-root"
        }),
    );
    assert_eq!(assert_ok(&absolute)["current_number"], 2);

    let outside = invoke(
        &ctx,
        "history_session_validate",
        json!({
            "workspace_root": workspace.path().parent().unwrap().to_string_lossy(),
            "repair": false
        }),
    );
    assert_eq!(
        assert_err(&outside)["error"]["code"],
        "PATH_OUTSIDE_WORKSPACE"
    );
}

#[test]
fn bootstrap_creates_next_file_returns_all_summaries_and_is_idempotent() {
    let (workspace, _harness, ctx) = test_context();
    prepare_history(workspace.path());

    let first = invoke(
        &ctx,
        "history_session_bootstrap",
        json!({"session_key": "current-chat", "title": "继续开发"}),
    );
    let first = assert_ok(&first);
    assert_eq!(first["is_new_session"], true);
    assert_eq!(first["session_key"], "current-chat");
    assert_eq!(first["session_key_source"], "explicit_session_key");
    assert_eq!(first["history_numbers"], json!([1, 2]));
    assert_eq!(first["history_count"], 2);
    assert_eq!(first["latest_completed_number"], 2);
    assert_eq!(first["latest_completed_path"], "docs/history-session/2.md");
    assert_eq!(first["current_number"], 3);
    assert_eq!(first["current_path"], "docs/history-session/3.md");
    assert_eq!(first["created"], true);
    assert_eq!(first["resumed"], false);
    assert_eq!(first["sequence_valid"], true);
    assert_eq!(first["history_read_mode"], "all_summaries_plus_latest_full");
    assert_eq!(first["full_history_included"], false);
    assert!(first["total_history_bytes"].as_u64().unwrap_or(0) > 0);
    assert_eq!(first["history_digest"].as_str().unwrap_or("").len(), 64);
    assert_eq!(first["persistence_mode"], "model_mediated_tool_calls");
    assert!(first["assistant_instructions"]
        .as_str()
        .unwrap_or("")
        .contains("history_session_checkpoint"));
    assert!(first["assistant_instructions"]
        .as_str()
        .unwrap_or("")
        .contains("After completing each user-requested task"));
    assert!(first["assistant_instructions"]
        .as_str()
        .unwrap_or("")
        .contains("before the final response"));
    assert!(first["assistant_instructions"]
        .as_str()
        .unwrap_or("")
        .contains("checkpoint returns ok=true"));
    assert_eq!(
        first["checkpoint_policy"]["required_before_final_response"],
        true
    );
    assert_eq!(
        first["checkpoint_policy"]["tool"],
        "history_session_checkpoint"
    );
    assert_eq!(first["checkpoint_policy"]["session_key"], "current-chat");
    assert_eq!(
        first["checkpoint_policy"]["expected_path"],
        "docs/history-session/3.md"
    );
    assert_eq!(first["checkpoint_policy"]["stable_target_required"], true);
    assert_eq!(
        first["required_next_actions"],
        json!([
            "read_all_history_summary",
            "read_latest_handoff",
            "verify_workspace_state",
            "execute_user_task",
            "checkpoint_after_each_completed_task"
        ])
    );
    assert_eq!(first["session_summaries"].as_array().unwrap().len(), 2);
    assert_eq!(first["session_summaries"][0]["number"], 1);
    assert_eq!(first["session_summaries"][1]["number"], 2);
    assert!(first["session_summaries"][0]["summary"]
        .as_str()
        .unwrap_or("")
        .contains("目标-第一阶段"));
    assert!(first["all_history_summary"]
        .as_str()
        .unwrap_or("")
        .contains("决定-第一阶段"));
    assert_eq!(
        first["latest_handoff"],
        history_file(2, "old-session-2", "第二阶段")
    );
    assert!(workspace.path().join("docs/history-session/3.md").is_file());
    let inherited = fs::read_to_string(workspace.path().join("docs/history-session/3.md"))
        .expect("read inherited summary");
    assert!(inherited.contains("## 继承的历史摘要"));
    assert!(inherited.contains("### 会话 1（docs/history-session/1.md）"));
    assert!(inherited.contains("### 会话 2（docs/history-session/2.md）"));
    assert!(first["inherited_summary"]
        .as_str()
        .unwrap_or("")
        .contains("目标-第一阶段"));

    let second = invoke(
        &ctx,
        "history_session_bootstrap",
        json!({"session_key": "current-chat", "title": "标题变化也不新建"}),
    );
    let second = assert_ok(&second);
    assert_eq!(second["current_number"], 3);
    assert_eq!(second["created"], false);
    assert_eq!(second["resumed"], true);
    assert!(!workspace.path().join("docs/history-session/4.md").exists());
}

#[test]
fn checkpoint_is_idempotent_updates_changed_turn_and_redacts_secrets() {
    let (workspace, _harness, ctx) = test_context();
    let boot = invoke(
        &ctx,
        "history_session_bootstrap",
        json!({"session_key": "checkpoint-chat"}),
    );
    assert_ok(&boot);

    let args = json!({
        "session_key": "checkpoint-chat",
        "expected_path": "docs/history-session/1.md",
        "turn_id": "turn-0001",
        "timestamp": "2026-07-17T11:00:00+08:00",
        "user_intent": "实现归档",
        "findings": ["接口已确认"],
        "decisions": ["使用 Bearer super-secret-token"],
        "files_changed": ["src/history.rs"],
        "tests": ["cargo test 通过"],
        "runtime_state": ["服务运行中"],
        "remaining_issues": ["无"],
        "next_actions": ["继续验证"],
        "notes": "password=hunter2"
    });
    let first = invoke(&ctx, "history_session_checkpoint", args.clone());
    let first = assert_ok(&first);
    assert_eq!(first["session_number"], 1);
    assert_eq!(first["path"], "docs/history-session/1.md");
    assert_eq!(first["session_key"], "checkpoint-chat");
    assert_eq!(first["expected_path"], "docs/history-session/1.md");
    assert_eq!(first["turn_id"], "turn-0001");
    assert_eq!(first["duplicate_ignored"], false);
    assert_eq!(first["content_hash"].as_str().unwrap_or("").len(), 64);
    assert!(!first["warnings"].as_array().unwrap().is_empty());

    let content = fs::read_to_string(workspace.path().join("docs/history-session/1.md"))
        .expect("read checkpoint");
    assert!(content.contains("[REDACTED]"));
    assert!(!content.contains("super-secret-token"));
    assert!(!content.contains("hunter2"));

    let duplicate = invoke(&ctx, "history_session_checkpoint", args.clone());
    let duplicate = assert_ok(&duplicate);
    assert_eq!(duplicate["duplicate_ignored"], true);
    let duplicate_content = fs::read_to_string(workspace.path().join("docs/history-session/1.md"))
        .expect("read duplicate checkpoint");
    assert_eq!(duplicate_content.matches("### turn-0001").count(), 1);

    let mut changed = args;
    changed["next_actions"] = json!(["运行完整回归"]);
    let updated = invoke(&ctx, "history_session_checkpoint", changed);
    let updated = assert_ok(&updated);
    assert_eq!(updated["duplicate_ignored"], false);
    assert_eq!(updated["updated"], true);
    let updated_content = fs::read_to_string(workspace.path().join("docs/history-session/1.md"))
        .expect("read updated checkpoint");
    assert_eq!(updated_content.matches("### turn-0001").count(), 1);
    let second_turn = invoke(
        &ctx,
        "history_session_checkpoint",
        json!({
            "session_key": "checkpoint-chat",
            "expected_path": "docs/history-session/1.md",
            "turn_id": "turn-0002",
            "user_intent": "second turn",
            "next_actions": ["deliver"]
        }),
    );
    assert_ok(&second_turn);
    let ordered = fs::read_to_string(workspace.path().join("docs/history-session/1.md"))
        .expect("read ordered checkpoints");
    assert!(ordered.find("### turn-0001").unwrap() < ordered.find("### turn-0002").unwrap());
    assert!(updated_content.contains("运行完整回归"));
    assert!(!updated_content.contains("继续验证"));
}

#[test]
fn checkpoint_rejects_sessions_that_were_not_bootstrapped() {
    let (_workspace, _harness, ctx) = test_context();
    let result = invoke(
        &ctx,
        "history_session_checkpoint",
        json!({
            "session_key": "unknown-chat",
            "expected_path": "docs/history-session/99.md",
            "turn_id": "turn-1"
        }),
    );
    let payload = assert_err(&result);
    assert_eq!(payload["error"]["code"], "SESSION_NOT_BOOTSTRAPPED");
}

#[test]
fn checkpoint_generates_a_stable_turn_id_when_the_client_omits_it() {
    let (_workspace, _harness, ctx) = test_context();
    let boot = invoke(
        &ctx,
        "history_session_bootstrap",
        json!({"session_key": "automatic-turn-id"}),
    );
    assert_ok(&boot);

    let args = json!({
        "session_key": "automatic-turn-id",
        "expected_path": "docs/history-session/1.md",
        "user_intent": "保存当前进度",
        "findings": ["工具目录缓存已确认"],
        "next_actions": ["重新配置连接后新开会话"]
    });
    let first_result = invoke(&ctx, "history_session_checkpoint", args.clone());
    let first = assert_ok(&first_result);
    let turn_id = first["turn_id"].as_str().expect("generated turn id");
    assert!(turn_id.starts_with("auto-"));

    let duplicate_result = invoke(&ctx, "history_session_checkpoint", args);
    let duplicate = assert_ok(&duplicate_result);
    assert_eq!(duplicate["turn_id"], turn_id);
    assert_eq!(duplicate["duplicate_ignored"], true);
}

#[test]
fn validate_reports_gaps_and_can_rebuild_a_missing_index() {
    let (workspace, _harness, ctx) = test_context();
    let dir = workspace.path().join("docs/history-session");
    fs::create_dir_all(&dir).expect("create history dir");
    fs::write(dir.join("1.md"), history_file(1, "gap-one", "一")).expect("write 1.md");
    fs::write(dir.join("3.md"), history_file(3, "gap-three", "三")).expect("write 3.md");
    fs::write(dir.join("bad.md"), "invalid").expect("write invalid file");
    fs::write(dir.join("4.md"), "").expect("write empty file");

    let readonly = invoke(&ctx, "history_session_validate", json!({"repair": false}));
    let readonly = assert_ok(&readonly);
    assert_eq!(readonly["sequence_valid"], false);
    assert_eq!(readonly["numbers"], json!([1, 3, 4]));
    assert_eq!(readonly["missing_numbers"], json!([2]));
    assert!(readonly["invalid_files"]
        .as_array()
        .unwrap()
        .contains(&json!("bad.md")));
    assert!(readonly["empty_files"]
        .as_array()
        .unwrap()
        .contains(&json!("4.md")));
    assert_eq!(readonly["latest_number"], 4);
    assert_eq!(readonly["latest_path"], "docs/history-session/4.md");
    assert!(!dir.join("index.json").exists());
    assert!(!dir.join("2.md").exists());
    fs::write(dir.join("index.json"), "{broken-json").expect("write broken index");

    let repaired = invoke(&ctx, "history_session_validate", json!({"repair": true}));
    let repaired = assert_ok(&repaired);
    assert_eq!(repaired["repaired"], true);
    assert_eq!(repaired["index_status"], "invalid");
    assert!(dir.join("index.json").is_file());
    assert!(!dir.join("2.md").exists());
    let index: Value = serde_json::from_str(
        &fs::read_to_string(dir.join("index.json")).expect("read rebuilt index"),
    )
    .expect("valid index json");
    assert_eq!(index["sessions"]["gap-one"]["number"], 1);
    assert_eq!(index["sessions"]["gap-three"]["number"], 3);
}

#[test]
fn history_dir_cannot_escape_the_workspace() {
    let (workspace, _harness, ctx) = test_context();
    let result = invoke(
        &ctx,
        "history_session_validate",
        json!({"history_dir": "../outside", "repair": false}),
    );
    let payload = assert_err(&result);
    assert_eq!(payload["error"]["code"], "PATH_OUTSIDE_WORKSPACE");
    let absolute = invoke(
        &ctx,
        "history_session_validate",
        json!({
            "history_dir": workspace.path().parent().unwrap().to_string_lossy(),
            "repair": false
        }),
    );
    let absolute = assert_err(&absolute);
    assert_eq!(absolute["error"]["code"], "PATH_OUTSIDE_WORKSPACE");
}

#[test]
fn concurrent_bootstrap_allocates_distinct_numbers() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let barrier = Arc::new(Barrier::new(2));
    let root = workspace.path().to_path_buf();

    let handles = ["parallel-a", "parallel-b"].map(|session_key| {
        let root = root.clone();
        let barrier = Arc::clone(&barrier);
        std::thread::spawn(move || {
            let harness = tempfile::tempdir().expect("harness tempdir");
            let ctx = ToolContext::for_test(root, harness.path().to_path_buf())
                .expect("parallel context");
            barrier.wait();
            let result = invoke(
                &ctx,
                "history_session_bootstrap",
                json!({"session_key": session_key}),
            );
            assert_ok(&result)["current_number"]
                .as_u64()
                .expect("current number")
        })
    });

    let mut numbers = handles
        .into_iter()
        .map(|handle| handle.join().expect("bootstrap thread"))
        .collect::<Vec<_>>();
    numbers.sort_unstable();
    assert_eq!(numbers, vec![1, 2]);
    assert!(workspace.path().join("docs/history-session/1.md").is_file());
    assert!(workspace.path().join("docs/history-session/2.md").is_file());
}
