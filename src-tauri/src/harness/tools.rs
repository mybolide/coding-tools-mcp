use serde_json::{json, Value};

use crate::tools::workspace::{tool_ok, WorkspaceError};
use crate::tools::ToolContext;

use super::model::TaskStatus;
use super::store::HarnessError;

pub const TOOL_NAMES: &[&str] = &[
    "project_state",
    "start_task",
    "update_task",
    "pause_task",
    "resume_task",
    "finish_task",
    "task_context",
    "list_task_events",
    "change_summary",
];

pub fn call(ctx: &ToolContext, name: &str, args: &Value) -> Result<Value, WorkspaceError> {
    let value = match name {
        "project_state" => project_state(ctx, args),
        "start_task" => start_task(ctx, args),
        "update_task" => update_task(ctx, args),
        "pause_task" => transition(ctx, args, TaskStatus::Paused),
        "resume_task" => transition(ctx, args, TaskStatus::Active),
        "finish_task" => finish_task(ctx, args),
        "task_context" => task_context(ctx, args),
        "list_task_events" => list_task_events(ctx, args),
        "change_summary" => change_summary(ctx, args),
        _ => return Err(tool_error("INVALID_ARGUMENT", "未知 Harness 工具")),
    }?;
    Ok(tool_ok(value))
}

fn project_state(ctx: &ToolContext, args: &Value) -> Result<Value, WorkspaceError> {
    let max_files = args.get("max_files").and_then(Value::as_u64).unwrap_or(200) as usize;
    serde_json::to_value(ctx.harness.project_state(max_files).map_err(map_error)?)
        .map_err(|e| tool_error("SERIALIZE_FAILED", e.to_string()))
}

fn start_task(ctx: &ToolContext, args: &Value) -> Result<Value, WorkspaceError> {
    let objective = args
        .get("objective")
        .and_then(Value::as_str)
        .ok_or_else(|| tool_error("INVALID_ARGUMENT", "objective 是必填项"))?;
    let task = ctx.harness.start_task(objective).map_err(map_error)?;
    Ok(json!({"task": task, "next": ["project_state", "task_context"]}))
}

fn update_task(ctx: &ToolContext, args: &Value) -> Result<Value, WorkspaceError> {
    let task_id = task_id(args)?;
    let completed_steps = string_list(args.get("completed_steps"))?;
    let pending_steps = string_list(args.get("pending_steps"))?;
    let task = ctx
        .harness
        .update_steps(task_id, completed_steps, pending_steps)
        .map_err(map_error)?;
    Ok(json!({"task": task}))
}

fn transition(
    ctx: &ToolContext,
    args: &Value,
    status: TaskStatus,
) -> Result<Value, WorkspaceError> {
    let task = ctx
        .harness
        .transition(task_id(args)?, status)
        .map_err(map_error)?;
    Ok(json!({"task": task}))
}

fn finish_task(ctx: &ToolContext, args: &Value) -> Result<Value, WorkspaceError> {
    let task_id = task_id(args)?;
    let allow_unverified = args
        .get("allow_unverified")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let status = if allow_unverified {
        TaskStatus::CompletedUnverified
    } else {
        TaskStatus::Verifying
    };
    let task = ctx.harness.transition(task_id, status).map_err(map_error)?;
    let summary = change_summary(ctx, &json!({"task_id": task_id}))?;
    Ok(json!({"task": task, "change_summary": summary}))
}

fn task_context(ctx: &ToolContext, args: &Value) -> Result<Value, WorkspaceError> {
    let task = if let Some(task_id) = args.get("task_id").and_then(Value::as_str) {
        Some(ctx.harness.task(task_id).map_err(map_error)?)
    } else {
        ctx.harness.current_task().map_err(map_error)?
    };
    let Some(task) = task else {
        return Ok(json!({"task": null, "message": "当前没有活动任务"}));
    };
    let events = ctx
        .harness
        .list_events(&task.id, 0, 100)
        .map_err(map_error)?;
    Ok(json!({"task": task, "events": events, "truncated": false}))
}

fn list_task_events(ctx: &ToolContext, args: &Value) -> Result<Value, WorkspaceError> {
    let task_id = task_id(args)?;
    let offset = args.get("cursor").and_then(Value::as_u64).unwrap_or(0) as usize;
    let limit = args
        .get("limit")
        .and_then(Value::as_u64)
        .unwrap_or(50)
        .clamp(1, 200) as usize;
    let events = ctx
        .harness
        .list_events(task_id, offset, limit)
        .map_err(map_error)?;
    Ok(json!({"events": events, "next_cursor": offset + events.len()}))
}

fn change_summary(ctx: &ToolContext, args: &Value) -> Result<Value, WorkspaceError> {
    let task = if let Some(task_id) = args.get("task_id").and_then(Value::as_str) {
        ctx.harness.task(task_id).map_err(map_error)?
    } else {
        ctx.harness
            .current_task()
            .map_err(map_error)?
            .ok_or_else(|| tool_error("TASK_STATE_REQUIRED", "没有可总结的活动任务"))?
    };
    let state = ctx.harness.project_state(200).map_err(map_error)?;
    let files = state
        .files
        .iter()
        .filter(|file| file.status != "unchanged")
        .cloned()
        .collect::<Vec<_>>();
    let events = ctx
        .harness
        .list_events(&task.id, 0, 100)
        .map_err(map_error)?;
    Ok(json!({
        "task_id": task.id,
        "objective": task.objective,
        "why": {"text": task.objective, "source": "task_objective"},
        "files": files,
        "evidence": events,
        "verification": [],
        "risks": [],
        "rollback_capability": "not_available_in_foundation"
    }))
}

fn task_id(args: &Value) -> Result<&str, WorkspaceError> {
    args.get("task_id")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| tool_error("INVALID_ARGUMENT", "task_id 是必填项"))
}

fn string_list(value: Option<&Value>) -> Result<Option<Vec<String>>, WorkspaceError> {
    let Some(value) = value else { return Ok(None) };
    let list = value
        .as_array()
        .ok_or_else(|| tool_error("INVALID_ARGUMENT", "步骤必须是字符串数组"))?
        .iter()
        .map(|item| {
            item.as_str()
                .map(str::to_string)
                .ok_or_else(|| tool_error("INVALID_ARGUMENT", "步骤必须是字符串数组"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Some(list))
}

fn map_error(error: HarnessError) -> WorkspaceError {
    tool_error(error.code(), error.to_string())
}

fn tool_error(code: &'static str, message: impl Into<String>) -> WorkspaceError {
    WorkspaceError::Tool {
        code,
        message: message.into(),
        category: "permission",
        retryable: matches!(
            code,
            "TASK_ALREADY_ACTIVE" | "FILE_CHANGED_EXTERNALLY" | "BASELINE_STALE"
        ),
    }
}
