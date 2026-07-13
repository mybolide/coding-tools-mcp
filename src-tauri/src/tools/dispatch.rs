use serde_json::{json, Value};

use crate::tools::context::ToolContext;
use crate::tools::policy::{validate_tool_arguments, PolicyError};
use crate::tools::workspace::{tool_err, tool_err_code, tool_ok, WorkspaceError};
use crate::tools::{exec, file, git, image_tool, patch, session};

fn policy_tool_err(err: PolicyError) -> Value {
    tool_err(WorkspaceError::Tool {
        code: "POLICY_REJECTED",
        message: err.0,
        category: "policy",
        retryable: false,
    })
}

/// **唯一工具执行入口**。MCP `tools/call` 与 Actions `POST /actions/{tool}` 必须且只能调用此函数。
/// 策略校验、分发、错误格式在此统一，两路传输层不得另做执行前校验（Actions 仅允许额外的暴露层 `validate_actions_exposure`）。
pub fn call_tool(ctx: &ToolContext, name: &str, args: &Value) -> Value {
    if let Err(e) = validate_tool_arguments(name, args, &ctx.policy) {
        return policy_tool_err(e);
    }

    if crate::harness::tools::TOOL_NAMES.contains(&name) {
        return match crate::harness::tools::call(ctx, name, args) {
            Ok(value) => value,
            Err(error) => tool_err(error),
        };
    }

    let task_id = if matches!(name, "apply_patch" | "exec_command") {
        let Some(task) = ctx.harness.current_task().ok().flatten() else {
            return tool_err_code(
                "TASK_STATE_REQUIRED",
                "写操作必须先启动一个 Harness 任务",
                "permission",
            );
        };
        if let Err(error) = ctx.harness.check_baseline(&task.id) {
            return tool_err_code(error.code(), error.to_string(), "permission");
        }
        let _ = ctx.harness.record_event(
            &task.id,
            "operation_started",
            Some(name),
            json!({"arguments_present": !args.is_null()}),
            json!({"ok": true}),
        );
        Some(task.id)
    } else {
        None
    };

    let ws = &ctx.workspace;
    let result = match name {
        "server_info" => server_info(ctx),
        "check_exec_environment" => check_exec_environment(ctx),
        "get_default_cwd" => get_default_cwd(ctx),
        "set_default_cwd" => set_default_cwd(ctx, args),
        "read_file" => file::read_file(ws, args),
        "list_dir" => file::list_dir(ws, args),
        "list_files" => file::list_files(ws, args),
        "search_text" => file::search_text(ws, args),
        "apply_patch" => patch::apply_patch(ws, args),
        "exec_command" => exec::exec_command(ctx, args),
        "read_output" => session::read_output(&ctx.sessions, args),
        "write_stdin" => session::write_stdin(&ctx.sessions, args),
        "kill_session" => session::kill_session(&ctx.sessions, args),
        "git_status" => git::git_status(ws, args),
        "git_diff" => git::git_diff(ws, args),
        "git_log" => git::git_log(ws, args),
        "git_show" => git::git_show(ws, args),
        "git_blame" => git::git_blame(ws, args),
        "view_image" => image_tool::view_image(ws, args),
        "request_permissions" => Ok(tool_ok(json!({
            "ok": false,
            "status": "unsupported",
            "error": {
                "code": "ELICITATION_UNSUPPORTED",
                "message": "Permission elicitation is not available for this client.",
                "category": "permission",
                "retryable": false,
                "details": { "requested": args }
            }
        }))),
        _ => {
            return tool_err_code(
                "INVALID_ARGUMENT",
                format!("Unknown tool: {name}"),
                "validation",
            )
        }
    };
    let output = match result {
        Ok(v) => v,
        Err(e) => tool_err(e),
    };
    if let Some(task_id) = task_id {
        let succeeded = output.get("ok").and_then(Value::as_bool) == Some(true);
        let _ = ctx.harness.record_event(
            &task_id,
            "operation_finished",
            Some(name),
            json!({"arguments_present": !args.is_null()}),
            json!({"ok": succeeded, "tool": name}),
        );
        if succeeded {
            let _ = ctx.harness.refresh_expected_state(&task_id);
        }
    }
    output
}

pub fn server_info(ctx: &ToolContext) -> Result<Value, WorkspaceError> {
    let tools = crate::tools::registry::exposed_tool_names(&ctx.tool_profile);
    Ok(tool_ok(json!({
        "server": "coding-tools-mcp",
        "title": "Coding Tools MCP",
        "version": "0.1.0",
        "protocol_version": "2025-06-18",
        "workspace": ctx.workspace.root_display(),
        "permission_mode": ctx.permission_mode,
        "default_cwd": ctx.default_cwd_display(),
        "network_allowed": ctx.policy.network_allowed(),
        "tool_profile": ctx.tool_profile,
        "auth_enabled": ctx.auth.auth_enabled(),
        "auth_type": ctx.auth.auth_type,
        "endpoint_path": "/mcp",
        "tools": tools,
        "tool_count": tools.len()
    })))
}

pub fn check_exec_environment(ctx: &ToolContext) -> Result<Value, WorkspaceError> {
    Ok(tool_ok(json!({
        "workspace": ctx.workspace.root_display(),
        "permission_mode": ctx.permission_mode,
        "network_allowed": ctx.policy.network_allowed(),
        "landlock_enabled": false,
        "global_tmp_write": if ctx.permission_mode == "dangerous" { "allowed" } else { "tmp-prefix" },
        "allowed_commands": ctx.policy.allowed_commands.iter().cloned().collect::<Vec<_>>(),
        "warnings": ["Linux Landlock filesystem confinement is unavailable on this build"]
    })))
}

pub fn get_default_cwd(ctx: &ToolContext) -> Result<Value, WorkspaceError> {
    Ok(tool_ok(json!({
        "workspace": ctx.workspace.root_display(),
        "default_cwd": ctx.default_cwd_display()
    })))
}

pub fn set_default_cwd(ctx: &ToolContext, args: &Value) -> Result<Value, WorkspaceError> {
    let path = args.get("path").and_then(Value::as_str).unwrap_or(".");
    let resolved = ctx.workspace.resolve_existing(path)?;
    if !resolved.path.is_dir() {
        return Err(WorkspaceError::not_a_directory(
            "Default cwd must be a directory",
        ));
    }
    ctx.set_default_cwd(resolved.path.clone());
    Ok(tool_ok(json!({
        "workspace": ctx.workspace.root_display(),
        "default_cwd": resolved.display
    })))
}
