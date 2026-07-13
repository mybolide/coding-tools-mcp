use std::fs;

use coding_tools_mcp_desktop_lib::harness::{Harness, TaskStatus};
use serde_json::json;

fn fixture() -> (tempfile::TempDir, std::path::PathBuf, std::path::PathBuf) {
    let temp = tempfile::tempdir().expect("创建临时目录");
    let workspace = temp.path().join("workspace");
    let harness_root = temp.path().join("harness");
    fs::create_dir_all(&workspace).expect("创建工作区");
    fs::write(workspace.join("README.md"), "初始内容\n").expect("写入夹具");
    (temp, workspace, harness_root)
}

#[test]
fn 任务创建会捕获基线并在重启后恢复() {
    let (_temp, workspace, harness_root) = fixture();
    let harness = Harness::new(workspace.clone(), harness_root.clone()).expect("创建 Harness");

    let task = harness.start_task("实现 Harness 基础能力").expect("启动任务");

    assert_eq!(task.status, TaskStatus::Active);
    assert_eq!(task.objective, "实现 Harness 基础能力");
    assert!(!task.baseline.worktree_fingerprint.is_empty());
    assert_eq!(harness.current_task().expect("读取任务").id, task.id);

    let restarted = Harness::new(workspace, harness_root).expect("重启 Harness");
    assert_eq!(restarted.current_task().expect("恢复任务").id, task.id);
}

#[test]
fn 同一工作区只允许一个可写任务且拒绝非法迁移() {
    let (_temp, workspace, harness_root) = fixture();
    let harness = Harness::new(workspace, harness_root).expect("创建 Harness");
    let task = harness.start_task("第一个任务").expect("启动任务");

    let duplicate = harness.start_task("第二个任务").expect_err("应拒绝第二个任务");
    assert_eq!(duplicate.code(), "TASK_ALREADY_ACTIVE");

    let invalid = harness
        .transition(&task.id, TaskStatus::Completed)
        .expect_err("active 不应直接完成");
    assert_eq!(invalid.code(), "INVALID_TASK_TRANSITION");

    let paused = harness
        .transition(&task.id, TaskStatus::Paused)
        .expect("暂停任务");
    assert_eq!(paused.status, TaskStatus::Paused);
    let resumed = harness
        .transition(&task.id, TaskStatus::Active)
        .expect("恢复任务");
    assert_eq!(resumed.status, TaskStatus::Active);
}

#[test]
fn 外部文件变化会被识别且操作会留下事件() {
    let (_temp, workspace, harness_root) = fixture();
    let harness = Harness::new(workspace.clone(), harness_root).expect("创建 Harness");
    let task = harness.start_task("验证外部变更").expect("启动任务");

    fs::write(workspace.join("README.md"), "外部修改\n").expect("模拟外部修改");
    let stale = harness.check_baseline(&task.id).expect_err("应识别外部修改");
    assert_eq!(stale.code(), "FILE_CHANGED_EXTERNALLY");

    harness
        .record_event(
            &task.id,
            "operation_finished",
            Some("read_file"),
            json!({"reason": "确认外部变更"}),
            json!({"ok": true}),
        )
        .expect("记录事件");
    let events = harness.list_events(&task.id, 0, 10).expect("读取事件");
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].tool_name.as_deref(), Some("read_file"));
}

#[test]
fn project_state包含分支任务和脏状态摘要() {
    let (_temp, workspace, harness_root) = fixture();
    let harness = Harness::new(workspace, harness_root).expect("创建 Harness");
    let task = harness.start_task("生成项目状态").expect("启动任务");

    let state = harness.project_state(20).expect("读取项目状态");

    assert_eq!(state.active_task_id.as_deref(), Some(task.id.as_str()));
    assert!(!state.files.is_empty());
    assert!(state.task.is_some());
}
