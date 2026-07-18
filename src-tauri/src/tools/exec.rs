use std::path::Path;
use std::time::{Duration, Instant};

#[cfg(windows)]
use std::os::windows::process::CommandExt;

use serde_json::{json, Value};
use tokio::process::Command;

use crate::tools::context::ToolContext;
use crate::tools::session::ExecSession;
use crate::tools::workspace::{tool_ok, WorkspaceError};

pub fn exec_command(ctx: &ToolContext, args: &Value) -> Result<Value, WorkspaceError> {
    let cmd = args
        .get("cmd")
        .and_then(Value::as_str)
        .ok_or_else(|| WorkspaceError::invalid_argument("cmd is required"))?;
    let workdir_raw = args
        .get("workdir")
        .or_else(|| args.get("cwd"))
        .and_then(Value::as_str)
        .unwrap_or(".");
    let workdir = ctx.workspace.resolve_existing(workdir_raw)?;
    if !workdir.path.is_dir() {
        return Err(WorkspaceError::not_a_directory(
            "workdir is not a directory",
        ));
    }
    let filesystem_scope = args
        .get("filesystem_scope")
        .and_then(Value::as_str)
        .unwrap_or("workspace")
        .to_string();
    validate_child_process_scope(ctx, args)?;
    if !ctx.policy.skip_permission_gates() {
        if let Some(result) = run_native_diagnostic(ctx, cmd, &workdir.path)? {
            let mut result = result;
            if let Some(object) = result.as_object_mut() {
                object.insert(
                    "filesystem_scope".into(),
                    Value::String(filesystem_scope.clone()),
                );
                object.insert("sandbox_enforced".into(), Value::Bool(false));
                object.insert(
                    "execution_boundary".into(),
                    Value::String("policy_only".into()),
                );
                object.insert("child_process".into(), Value::Bool(false));
                object.insert("transport_ok".into(), Value::Bool(true));
                object.insert("command_ok".into(), Value::Bool(true));
            }
            return Ok(tool_ok(result));
        }
    }
    let timeout_ms = args
        .get("timeout_ms")
        .and_then(Value::as_u64)
        .unwrap_or(30_000);
    let max_output = args
        .get("max_output_bytes")
        .and_then(Value::as_u64)
        .unwrap_or(65_536) as usize;
    let yield_ms = args
        .get("yield_time_ms")
        .and_then(Value::as_u64)
        .unwrap_or(1000)
        .min(30_000);
    let tty = args.get("tty").and_then(Value::as_bool).unwrap_or(false);
    let stdin_text = args.get("stdin").and_then(Value::as_str).unwrap_or("");

    let result = tauri::async_runtime::block_on(async {
        run_command(
            ctx,
            cmd,
            &workdir.path,
            Duration::from_millis(timeout_ms),
            Duration::from_millis(yield_ms),
            max_output,
            tty,
            stdin_text,
            args.get("env"),
        )
        .await
    });

    match result {
        Ok(mut out) => {
            if let Some(object) = out.as_object_mut() {
                object.insert("filesystem_scope".into(), Value::String(filesystem_scope));
                object.insert("sandbox_enforced".into(), Value::Bool(false));
                object.insert(
                    "execution_boundary".into(),
                    Value::String("policy_only".into()),
                );
                object.insert("child_process".into(), Value::Bool(true));
                object.insert(
                    "execution_mode".into(),
                    Value::String(if ctx.policy.skip_permission_gates() {
                        "shell".into()
                    } else {
                        "direct".into()
                    }),
                );
                object.insert(
                    "permission_gates_skipped".into(),
                    Value::Bool(ctx.policy.skip_permission_gates()),
                );
            }
            Ok(tool_ok(out))
        }
        Err(error) => match execution_failure_result(&error, cmd, &workdir.path, &filesystem_scope)
        {
            Some(result) => Ok(tool_ok(result)),
            None => Err(error),
        },
    }
}

fn validate_child_process_scope(ctx: &ToolContext, args: &Value) -> Result<(), WorkspaceError> {
    let scope = args
        .get("filesystem_scope")
        .and_then(Value::as_str)
        .unwrap_or("workspace");
    match scope {
        "workspace" => Ok(()),
        "host" if ctx.policy.skip_permission_gates() => Ok(()),
        "host" => Err(WorkspaceError::ToolDetails {
            code: "EXTERNAL_EXECUTION_NOT_ALLOWED",
            message: "exec_command 只允许在 Workspace 内执行，Workspace 外执行已禁用。".into(),
            category: "permission",
            retryable: false,
            details: json!({
                "stage": "policy",
                "filesystem_scope": "host",
                "sandbox_enforced": false,
                "recoverable": false,
                "suggestion": "将 filesystem_scope 设置为 workspace，并在当前 Workspace 内执行"
            }),
        }),
        _ => Err(WorkspaceError::invalid_argument(
            "filesystem_scope must be workspace",
        )),
    }
}

fn run_native_diagnostic(
    ctx: &ToolContext,
    cmd: &str,
    cwd: &Path,
) -> Result<Option<Value>, WorkspaceError> {
    let parts = shell_words::split(cmd)
        .map_err(|_| WorkspaceError::invalid_argument("Invalid command syntax"))?;
    if parts.is_empty() {
        return Ok(None);
    }

    let command = parts[0].to_ascii_lowercase();
    let stdout = match command.as_str() {
        "pwd" if parts.len() == 1 => Some(format!("{}\n", cwd.display())),
        "ls" | "dir" => Some(list_directory(ctx, cwd, &parts[1..])?),
        "which" if parts.len() == 2 => {
            let path = which::which(&parts[1]).map_err(|_| WorkspaceError::Tool {
                code: "COMMAND_NOT_FOUND",
                message: format!("Program not found on PATH: {}", parts[1]),
                category: "runtime",
                retryable: false,
            })?;
            Some(format!("{}\n", path.display()))
        }
        "echo" => Some(format!("{}\n", parts[1..].join(" "))),
        _ => None,
    };

    Ok(stdout.map(|stdout| {
        json!({
            "command": cmd,
            "resolved_cwd": cwd.display().to_string(),
            "status": "exited",
            "termination_reason": "exited",
            "recoverable": false,
            "suggestion": "命令已完成",
            "exit_code": 0,
            "stdout": stdout,
            "stderr": "",
            "stdout_truncated": false,
            "stderr_truncated": false,
            "duration_ms": 0,
            "elapsed_ms": 0,
            "execution_mode": "native_builtin",
            "command_runner": "native_builtin",
            "warnings": ["native diagnostic without child process"]
        })
    }))
}

fn list_directory(
    ctx: &ToolContext,
    cwd: &Path,
    args: &[String],
) -> Result<String, WorkspaceError> {
    let target = match args {
        [] => cwd.to_path_buf(),
        [path] => ctx.workspace.resolve_existing(path)?.path,
        _ => {
            return Err(WorkspaceError::invalid_argument(
                "ls/dir accepts at most one directory path",
            ))
        }
    };
    if !target.is_dir() {
        return Err(WorkspaceError::not_a_directory(
            "ls/dir target is not a directory",
        ));
    }

    let mut entries = std::fs::read_dir(target)
        .map_err(|error| WorkspaceError::ToolDetails {
            code: "DIRECTORY_READ_FAILED",
            message: format!("Failed to read directory: {error}"),
            category: "runtime",
            retryable: true,
            details: json!({
                "stage": "native_builtin",
                "reason": "directory_read_failed",
                "retryable": true
            }),
        })?
        .filter_map(Result::ok)
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    entries.sort_unstable();
    Ok(if entries.is_empty() {
        String::new()
    } else {
        format!("{}\n", entries.join("\n"))
    })
}

#[allow(clippy::too_many_arguments)]
async fn run_command(
    ctx: &ToolContext,
    cmd: &str,
    cwd: &Path,
    limit: Duration,
    yield_time: Duration,
    max_output: usize,
    tty: bool,
    stdin_text: &str,
    env: Option<&Value>,
) -> Result<Value, WorkspaceError> {
    let start = Instant::now();

    // macOS strips DYLD_* variables while starting the platform shell.  Keep
    // dangerous-mode env arguments effective for the actual child executable
    // by exporting them from inside the shell as well.  This is needed for
    // Homebrew runtimes whose dependent dylibs are supplied through a
    // DYLD_LIBRARY_PATH override; ordinary environment variables continue to
    // use Command::env below.
    let shell_command = if ctx.policy.skip_permission_gates() {
        shell_command_with_dynamic_loader_env(cmd, env)?
    } else {
        cmd.to_string()
    };

    let mut command = if ctx.policy.skip_permission_gates() {
        command_for_shell(&shell_command)
    } else {
        let (program, args) = parse_and_resolve(cmd, cwd, ctx.workspace.root(), &ctx.policy)?;
        command_for_program(&program, &args)
    };
    command
        .current_dir(platform_command_path(cwd))
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());

    if let Some(env) = env {
        let env = env
            .as_object()
            .ok_or_else(|| WorkspaceError::invalid_argument("env must be an object"))?;
        for (key, value) in env {
            let value = value
                .as_str()
                .map(str::to_string)
                .unwrap_or_else(|| value.to_string());
            command.env(key, value);
        }
    }

    #[cfg(windows)]
    command
        .env("PYTHONUTF8", "1")
        .env("PYTHONIOENCODING", "utf-8")
        .env("PYTHONLEGACYWINDOWSSTDIO", "0");

    let child = command.spawn().map_err(|e| WorkspaceError::ToolDetails {
        code: "COMMAND_SPAWN_FAILED",
        message: format!("Failed to start command: {e}"),
        category: "runtime",
        retryable: true,
        details: json!({
            "termination_reason": "spawn_failed",
            "recoverable": true,
            "suggestion": "检查命令路径、权限和运行时环境后重试"
        }),
    })?;

    let session = ctx.sessions.insert(ExecSession::new_with_mode(child, tty));
    session.spawn_readers().await;
    let deadline = start + limit;

    if yield_time.is_zero() {
        let snapshot = session.snapshot(max_output);
        spawn_timeout_monitor(session.clone(), deadline);
        return Ok(merge_exec_result(snapshot, start, cmd, cwd, true));
    }

    if !tty && !stdin_text.is_empty() {
        let mut stdin_guard = session.stdin.lock().await;
        if let Some(stdin) = stdin_guard.as_mut() {
            use tokio::io::AsyncWriteExt;
            if !stdin_text.is_empty() {
                stdin
                    .write_all(stdin_text.as_bytes())
                    .await
                    .map_err(|_| WorkspaceError::Tool {
                        code: "SESSION_CLOSED",
                        message: "Failed to write stdin.".into(),
                        category: "runtime",
                        retryable: false,
                    })?;
            }
            let _ = stdin.shutdown().await;
        }
        *stdin_guard = None;
        session.mark_stdin_closed();
    }

    loop {
        session.refresh_status().await;
        if session.has_exited() {
            session.wait_for_readers().await;
            let snapshot = session.snapshot(max_output);
            ctx.sessions.remove(&session.session_id);
            return Ok(merge_exec_result(snapshot, start, cmd, cwd, false));
        }
        if !tty && Instant::now() >= deadline {
            session.mark_termination_reason("timeout");
            session.kill_and_wait().await;
            session.refresh_status().await;
            session.wait_for_readers().await;
            let snapshot = session.snapshot(max_output);
            return Err(WorkspaceError::ToolDetails {
                code: "TIMEOUT",
                message: "Command timed out.".into(),
                category: "runtime",
                retryable: true,
                details: json!({
                    "termination_reason": "timeout",
                    "recoverable": true,
                    "suggestion": "读取 output_refs，调整 timeout_ms 后重试",
                    "session": snapshot
                }),
            });
        }
        if Instant::now() - start >= yield_time || tty {
            let snapshot = session.snapshot(max_output);
            spawn_timeout_monitor(session.clone(), deadline);
            return Ok(merge_exec_result(snapshot, start, cmd, cwd, true));
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

fn shell_command_with_dynamic_loader_env(
    cmd: &str,
    env: Option<&Value>,
) -> Result<String, WorkspaceError> {
    #[cfg(not(target_os = "macos"))]
    {
        let _ = env;
        return Ok(cmd.to_string());
    }

    #[cfg(target_os = "macos")]
    {
        let Some(env) = env else {
            return Ok(cmd.to_string());
        };
        let env = env
            .as_object()
            .ok_or_else(|| WorkspaceError::invalid_argument("env must be an object"))?;
        let mut exports = Vec::new();
        for (key, value) in env {
            if !key.starts_with("DYLD_") || !is_shell_identifier(key) {
                continue;
            }
            let value = value
                .as_str()
                .map(str::to_string)
                .unwrap_or_else(|| value.to_string());
            exports.push(format!("export {key}={}", shell_single_quote(&value)));
        }
        if exports.is_empty() {
            Ok(cmd.to_string())
        } else {
            Ok(format!("{}; {}", exports.join("; "), cmd))
        }
    }
}

#[cfg(target_os = "macos")]
fn is_shell_identifier(value: &str) -> bool {
    let mut chars = value.chars();
    matches!(chars.next(), Some('_' | 'A'..='Z' | 'a'..='z'))
        && chars.all(|ch| matches!(ch, '_' | 'A'..='Z' | 'a'..='z' | '0'..='9'))
}

#[cfg(target_os = "macos")]
fn shell_single_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

fn spawn_timeout_monitor(session: std::sync::Arc<ExecSession>, deadline: Instant) {
    tauri::async_runtime::spawn(async move {
        let remaining = deadline.saturating_duration_since(Instant::now());
        tokio::time::sleep(remaining).await;
        session.refresh_status().await;
        if !session.has_exited() {
            session.mark_termination_reason("timeout");
            session.kill_and_wait().await;
            session.refresh_status().await;
            session.wait_for_readers().await;
        }
    });
}

pub fn exec_health_check(ctx: &ToolContext) -> Result<Value, WorkspaceError> {
    let start = Instant::now();
    let cwd = ctx.workspace.root().to_path_buf();
    #[cfg(windows)]
    let probe = r#"cmd.exe /d /c "echo exec-health && echo exec-health-stderr 1>&2""#;
    #[cfg(not(windows))]
    let probe = r#"sh -c "printf exec-health; printf exec-health-stderr >&2""#;

    let result = tauri::async_runtime::block_on(run_command(
        ctx,
        probe,
        &cwd,
        Duration::from_secs(5),
        Duration::from_secs(5),
        16_384,
        false,
        "",
        None,
    ));

    let mut response = json!({
        "worker": {"alive": true},
        "session_create": false,
        "command_run": false,
        "stdout_capture": false,
        "stderr_capture": false,
        "duration_ms": start.elapsed().as_millis(),
        "next_actions": []
    });

    match result {
        Ok(snapshot) => {
            let session_created = snapshot.get("session_id").is_some();
            let command_run = snapshot.get("exit_code").and_then(Value::as_i64) == Some(0);
            let stdout_capture = snapshot
                .get("stdout")
                .and_then(Value::as_str)
                .is_some_and(|value| value.contains("exec-health"));
            let stderr_capture = snapshot
                .get("stderr")
                .and_then(Value::as_str)
                .is_some_and(|value| value.contains("exec-health-stderr"));
            let healthy = session_created && command_run && stdout_capture && stderr_capture;
            response["session_create"] = Value::Bool(session_created);
            response["command_run"] = Value::Bool(command_run);
            response["stdout_capture"] = Value::Bool(stdout_capture);
            response["stderr_capture"] = Value::Bool(stderr_capture);
            response["status"] = Value::String(if healthy { "success" } else { "error" }.into());
            response["summary"] = Value::String(if healthy {
                "exec worker、session、命令执行和 stdout/stderr 捕获均正常".into()
            } else {
                "exec health check 未通过，请查看 probe 结果".into()
            });
            response["probe"] = snapshot;
            if !healthy {
                response["next_actions"] = json!(["检查 exec worker 日志", "重启运行时"]);
            }
        }
        Err(error) => {
            response["status"] = Value::String("error".into());
            response["summary"] = Value::String("exec session 创建或探针执行失败".into());
            response["error"] = error.to_error_value();
            response["next_actions"] = json!(["检查 exec worker 日志", "重启运行时"]);
        }
    }
    response["duration_ms"] = json!(start.elapsed().as_millis());
    Ok(tool_ok(response))
}

fn execution_failure_result(
    error: &WorkspaceError,
    command: &str,
    cwd: &Path,
    filesystem_scope: &str,
) -> Option<Value> {
    let code = match &error {
        WorkspaceError::Tool { code, .. } | WorkspaceError::ToolDetails { code, .. } => *code,
    };
    if !matches!(
        code,
        "COMMAND_REJECTED" | "COMMAND_SPAWN_FAILED" | "TIMEOUT"
    ) {
        return None;
    }

    let error_value = error.to_error_value();
    let details = error_value
        .get("details")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let mut result = details.get("session").cloned().unwrap_or_else(|| {
        json!({
            "status": "spawn_failed",
            "termination_reason": "spawn_failed",
            "recoverable": error_value["retryable"].as_bool().unwrap_or(false),
            "exit_code": Value::Null,
            "stdout": "",
            "stderr": "",
            "stdout_truncated": false,
            "stderr_truncated": false
        })
    });
    if let Some(object) = result.as_object_mut() {
        object.insert("command".into(), json!(command));
        object.insert("resolved_cwd".into(), json!(cwd.display().to_string()));
        object.insert("execution_mode".into(), json!("direct"));
        object.insert("filesystem_scope".into(), json!(filesystem_scope));
        object.insert("sandbox_enforced".into(), Value::Bool(false));
        object.insert("execution_boundary".into(), json!("policy_only"));
        object.insert("child_process".into(), Value::Bool(true));
        object.insert("transport_ok".into(), Value::Bool(true));
        object.insert("command_ok".into(), Value::Bool(false));
        object.insert("error".into(), error_value);
        if code == "TIMEOUT" {
            object.insert("termination_reason".into(), json!("timeout"));
        } else {
            object.insert("status".into(), json!("spawn_failed"));
            object.insert("termination_reason".into(), json!("spawn_failed"));
        }
    }
    Some(result)
}

fn merge_exec_result(
    mut snapshot: Value,
    start: Instant,
    command: &str,
    cwd: &Path,
    keep_session: bool,
) -> Value {
    if let Some(obj) = snapshot.as_object_mut() {
        let duration_ms = start.elapsed().as_millis();
        obj.insert("command".into(), json!(command));
        obj.insert("resolved_cwd".into(), json!(cwd.display().to_string()));
        obj.insert("duration_ms".into(), json!(duration_ms));
        obj.insert("elapsed_ms".into(), json!(duration_ms));
        obj.insert("transport_ok".into(), Value::Bool(true));
        let command_ok = match obj
            .get("termination_reason")
            .and_then(Value::as_str)
            .unwrap_or("running")
        {
            "exited" => obj
                .get("exit_code")
                .and_then(Value::as_i64)
                .map(|exit_code| exit_code == 0)
                .or(Some(false)),
            "running" => None,
            _ => Some(false),
        };
        obj.insert(
            "command_ok".into(),
            command_ok.map(Value::Bool).unwrap_or(Value::Null),
        );
        obj.insert("execution_mode".into(), json!("direct"));
        obj.insert(
            "warnings".into(),
            json!(if keep_session {
                vec!["session retained for read_output/write_stdin/kill_session"]
            } else {
                vec!["direct execution without shell"]
            }),
        );
    }
    snapshot
}

fn parse_and_resolve(
    cmd: &str,
    cwd: &Path,
    workspace_root: &Path,
    policy: &crate::tools::policy::PolicySettings,
) -> Result<(String, Vec<String>), WorkspaceError> {
    let parts = shell_words::split(cmd)
        .map_err(|_| WorkspaceError::invalid_argument("Invalid command syntax"))?;
    if parts.is_empty() {
        return Err(WorkspaceError::invalid_argument("Empty command"));
    }

    let program = resolve_program(&parts[0], cwd, workspace_root, policy)?;
    Ok((program, parts[1..].to_vec()))
}

fn command_for_shell(command: &str) -> Command {
    #[cfg(windows)]
    {
        let mut shell = Command::new("cmd.exe");
        shell.args(["/d", "/s", "/c", command]);
        return shell;
    }

    let mut shell = Command::new("/bin/sh");
    shell.args(["-c", command]);
    shell
}

fn resolve_program(
    raw: &str,
    cwd: &Path,
    workspace_root: &Path,
    policy: &crate::tools::policy::PolicySettings,
) -> Result<String, WorkspaceError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(WorkspaceError::invalid_argument("Empty program"));
    }

    let explicit_path = trimmed.contains(['/', '\\']);
    let candidate = if Path::new(trimmed).is_absolute() {
        Path::new(trimmed).to_path_buf()
    } else {
        cwd.join(trimmed)
    };
    if candidate.is_file() {
        let resolved = candidate.canonicalize().map_err(|_| WorkspaceError::Tool {
            code: "COMMAND_REJECTED",
            message: format!("Program not found: {trimmed}"),
            category: "runtime",
            retryable: false,
        })?;
        let canonical_workspace =
            workspace_root
                .canonicalize()
                .map_err(|_| WorkspaceError::Tool {
                    code: "COMMAND_REJECTED",
                    message: "Workspace root is unavailable".into(),
                    category: "runtime",
                    retryable: true,
                })?;
        if !resolved.starts_with(&canonical_workspace) {
            return Err(WorkspaceError::Tool {
                code: "EXECUTABLE_OUTSIDE_WORKSPACE",
                message: format!("Workspace 外可执行文件被拒绝: {trimmed}"),
                category: "security",
                retryable: false,
            });
        }
        let extension = resolved
            .extension()
            .and_then(|value| value.to_str())
            .map(|value| format!(".{}", value.to_ascii_lowercase()))
            .unwrap_or_default();
        if policy.workspace_local_entries
            && (extension.is_empty() || policy.workspace_script_extensions.contains(&extension))
        {
            return Ok(resolved.to_string_lossy().into_owned());
        }
        return Err(WorkspaceError::Tool {
            code: "COMMAND_REJECTED",
            message: format!("Workspace 本地入口未获允许: {trimmed}"),
            category: "policy",
            retryable: false,
        });
    }

    if explicit_path {
        return Err(WorkspaceError::Tool {
            code: "COMMAND_REJECTED",
            message: format!("Program not found: {trimmed}"),
            category: "runtime",
            retryable: false,
        });
    }

    which::which(trimmed)
        .map(|p| p.to_string_lossy().into_owned())
        .map_err(|_| WorkspaceError::Tool {
            code: "COMMAND_REJECTED",
            message: format!("Program not found on PATH: {trimmed}"),
            category: "runtime",
            retryable: false,
        })
}

#[cfg(test)]
#[allow(clippy::items_after_test_module)]
mod tests {
    use super::*;
    use crate::tools::context::ToolContext;
    use crate::tools::dispatch::call_tool;
    use serde_json::json;
    use tempfile::tempdir;

    fn assert_failure_result(error: WorkspaceError, expected_code: &str) {
        let result = execution_failure_result(
            &error,
            "missing-command",
            Path::new("C:/workspace"),
            "workspace",
        )
        .expect("应转换为统一执行结果");
        assert_eq!(result["transport_ok"], true);
        assert_eq!(result["command_ok"], false);
        assert_eq!(result["status"], "spawn_failed");
        assert_eq!(result["error"]["code"], expected_code);
    }

    fn dangerous_context(workspace: &Path, harness: &Path) -> ToolContext {
        let mut context =
            ToolContext::for_test(workspace.to_path_buf(), harness.to_path_buf()).expect("context");
        context.policy.permission_mode = "dangerous".into();
        context.permission_mode = "dangerous".into();
        context.workspace.set_unrestricted(true);
        context
    }

    #[cfg(unix)]
    #[test]
    fn dangerous_mode_executes_host_shell_with_environment_and_absolute_read() {
        let workspace = tempdir().expect("workspace");
        let host = tempdir().expect("host");
        let harness = tempdir().expect("harness");
        let context = dangerous_context(workspace.path(), harness.path());

        let output = call_tool(
            &context,
            "exec_command",
            &json!({
                "cmd": "printf '%s|%s' \"$CODEX_DANGER_PROBE\" \"$DYLD_LIBRARY_PATH\" > dangerous-probe.txt; printf '\\n' >> dangerous-probe.txt",
                "workdir": host.path().to_string_lossy(),
                "filesystem_scope": "host",
                "timeout_ms": 3_600_000,
                "yield_time_ms": 10_000,
                "env": {
                    "CODEX_DANGER_PROBE": "host-shell-ok",
                    "DYLD_LIBRARY_PATH": "/tmp/codex-dangerous-dyld"
                }
            }),
        );
        assert_eq!(output["ok"], true, "{output}");
        assert_eq!(output["command_ok"], true, "{output}");
        assert_eq!(output["filesystem_scope"], "host");
        assert_eq!(output["execution_mode"], "shell");
        assert_eq!(output["permission_gates_skipped"], true);

        let read = call_tool(
            &context,
            "read_file",
            &json!({
                "path": host.path().join("dangerous-probe.txt").to_string_lossy()
            }),
        );
        assert_eq!(read["ok"], true, "{read}");
        assert_eq!(read["content"], "host-shell-ok|/tmp/codex-dangerous-dyld\n");
    }

    #[test]
    fn 程序不存在时返回统一执行结果() {
        assert_failure_result(
            WorkspaceError::Tool {
                code: "COMMAND_REJECTED",
                message: "Program not found on PATH: missing-command".into(),
                category: "runtime",
                retryable: false,
            },
            "COMMAND_REJECTED",
        );
    }

    #[test]
    fn 启动失败时返回统一执行结果() {
        assert_failure_result(
            WorkspaceError::ToolDetails {
                code: "COMMAND_SPAWN_FAILED",
                message: "Failed to start command".into(),
                category: "runtime",
                retryable: true,
                details: json!({"recoverable": true}),
            },
            "COMMAND_SPAWN_FAILED",
        );
    }

    #[test]
    fn resolves_an_arbitrarily_named_workspace_local_entry() {
        let workspace = tempdir().expect("workspace");
        let entry = workspace.path().join("scripts").join("anything.cmd");
        std::fs::create_dir_all(entry.parent().expect("parent")).expect("scripts");
        std::fs::write(&entry, "echo test").expect("entry");
        let resolved = resolve_program(
            "scripts/anything.cmd",
            workspace.path(),
            workspace.path(),
            &crate::tools::policy::PolicySettings::default(),
        )
        .expect("workspace entry resolves");
        assert_eq!(
            std::path::Path::new(&resolved),
            entry.canonicalize().unwrap()
        );
    }

    #[cfg(windows)]
    #[test]
    fn windows_scripts_use_their_platform_runners() {
        let batch = command_for_program("C:/workspace/run-anything.cmd", &[]);
        assert_eq!(batch.as_std().get_program().to_string_lossy(), "cmd.exe");
        assert!(batch.as_std().get_args().any(|arg| arg == "/c"));
        assert_eq!(
            windows_batch_command_line(
                r"\\?\C:\workspace\Life Brain\run & tooling.cmd",
                &["argument & value".to_string()]
            ),
            r#"call "C:\workspace\Life Brain\run & tooling.cmd" "argument & value""#
        );

        let script = command_for_program("C:/workspace/run-anything.ps1", &[]);
        let runner = script
            .as_std()
            .get_program()
            .to_string_lossy()
            .to_ascii_lowercase();
        assert!(runner.contains("powershell") || runner.contains("pwsh"));
        assert!(script.as_std().get_args().any(|arg| arg == "-File"));
    }

    #[cfg(windows)]
    #[test]
    fn windows_workspace_scripts_and_python_unicode_execute_successfully() {
        let workspace = tempdir().expect("workspace");
        let harness = tempdir().expect("harness");
        std::fs::write(
            workspace.path().join("any-name.cmd"),
            "@echo tooling-cmd-ok\r\n",
        )
        .expect("cmd script");
        std::fs::write(
            workspace.path().join("any-name.ps1"),
            "Write-Output 'tooling-powershell-ok'\r\n",
        )
        .expect("powershell script");
        std::fs::write(
            workspace.path().join("workflow_probe.py"),
            "print('workflow-ok')\n",
        )
        .expect("python module");
        let ctx =
            ToolContext::for_test(workspace.path().to_path_buf(), harness.path().to_path_buf())
                .expect("context");

        for command in [
            "any-name.cmd",
            "any-name.ps1",
            "cmd /c echo tooling-cmd-ok",
            "powershell -NoProfile -Command \"Write-Output tooling-powershell-ok\"",
            "python -c \"print('中文输出正常 ✅')\"",
        ] {
            let output = call_tool(
                &ctx,
                "exec_command",
                &json!({ "cmd": command, "timeout_ms": 10_000, "yield_time_ms": 10_000 }),
            );
            assert_eq!(output["ok"], true, "{command}: {output}");
            assert_eq!(output["command_ok"], true, "{command}: {output}");
        }

        for _ in 0..10 {
            let output = call_tool(
                &ctx,
                "exec_command",
                &json!({ "cmd": "python -m workflow_probe", "timeout_ms": 10_000 }),
            );
            assert_eq!(output["command_ok"], true, "{output}");
            assert!(output["stdout"]
                .as_str()
                .unwrap_or_default()
                .contains("workflow-ok"));
        }
    }

    #[cfg(windows)]
    #[test]
    fn windows_batch_scripts_preserve_space_paths_and_arguments() {
        let parent = tempdir().expect("workspace parent");
        let workspace = parent.path().join("Life Brain 中文");
        std::fs::create_dir_all(&workspace).expect("workspace");
        let harness = tempdir().expect("harness");
        let ctx = ToolContext::for_test(workspace.clone(), harness.path().to_path_buf())
            .expect("context");

        for extension in ["cmd", "bat"] {
            let script_name = format!("run & tooling.{extension}");
            std::fs::write(
                workspace.join(&script_name),
                "@echo off\r\nif not \"%~1\"==\"argument & value\" exit /b 7\r\necho tooling-space-path-ok\r\n",
            )
            .expect("batch script");

            let command = format!(r#""{script_name}" "argument & value""#);
            let output = call_tool(
                &ctx,
                "exec_command",
                &json!({ "cmd": command, "timeout_ms": 10_000, "yield_time_ms": 10_000 }),
            );
            assert_eq!(output["command_ok"], true, "{script_name}: {output}");
            let stdout = output["stdout"].as_str().unwrap_or_default();
            assert!(
                stdout.contains("tooling-space-path-ok"),
                "{script_name}: {output}"
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn unix_workspace_scripts_preserve_space_paths_and_arguments() {
        use std::os::unix::fs::PermissionsExt;

        let parent = tempdir().expect("workspace parent");
        let workspace = parent.path().join("Life Brain 中文");
        std::fs::create_dir_all(&workspace).expect("workspace");
        let harness = tempdir().expect("harness");
        let script_name = "run tooling";
        let script_path = workspace.join(script_name);
        std::fs::write(
            &script_path,
            "#!/bin/sh\nprintf 'tooling-space-path-ok\\n'\nprintf 'argument=[%s]\\n' \"$1\"\n",
        )
        .expect("shell script");
        let mut permissions = std::fs::metadata(&script_path)
            .expect("script metadata")
            .permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&script_path, permissions).expect("script executable");

        let ctx = ToolContext::for_test(workspace, harness.path().to_path_buf()).expect("context");
        let command = format!(r#""{script_name}" "argument with spaces""#);
        let output = call_tool(
            &ctx,
            "exec_command",
            &json!({ "cmd": command, "timeout_ms": 10_000, "yield_time_ms": 10_000 }),
        );
        assert_eq!(output["command_ok"], true, "{output}");
        let stdout = output["stdout"].as_str().unwrap_or_default();
        assert!(stdout.contains("tooling-space-path-ok"), "{output}");
        assert!(
            stdout.contains("argument=[argument with spaces]"),
            "{output}"
        );
    }
}

fn command_for_program(program: &str, args: &[String]) -> Command {
    #[cfg(windows)]
    {
        let extension = Path::new(program)
            .extension()
            .and_then(|value| value.to_str())
            .map(str::to_ascii_lowercase);
        match extension.as_deref() {
            Some("bat") | Some("cmd") => {
                let mut command = Command::new("cmd.exe");
                command.args(["/d", "/s", "/c"]);
                command
                    .as_std_mut()
                    .raw_arg(windows_batch_command_line(program, args));
                return command;
            }
            Some("ps1") => {
                let shell = which::which("pwsh")
                    .or_else(|_| which::which("powershell"))
                    .unwrap_or_else(|_| std::path::PathBuf::from("powershell.exe"));
                let mut command = Command::new(shell);
                command
                    .args([
                        "-NoLogo",
                        "-NoProfile",
                        "-NonInteractive",
                        "-ExecutionPolicy",
                        "Bypass",
                        "-File",
                        windows_command_path(program).as_str(),
                    ])
                    .args(args);
                return command;
            }
            _ => {}
        }
    }

    let mut command = Command::new(program);
    command.args(args);
    command
}

#[cfg(windows)]
fn windows_batch_command_line(program: &str, args: &[String]) -> String {
    let mut command_line = String::from("call ");
    command_line.push_str(&windows_batch_token(&windows_command_path(program)));
    for arg in args {
        command_line.push(' ');
        command_line.push_str(&windows_batch_token(arg));
    }
    command_line
}

#[cfg(windows)]
fn windows_batch_token(value: &str) -> String {
    format!("\"{}\"", value.replace('"', "\"\""))
}

fn platform_command_path(path: &Path) -> std::path::PathBuf {
    #[cfg(windows)]
    {
        std::path::PathBuf::from(windows_command_path(&path.to_string_lossy()))
    }
    #[cfg(not(windows))]
    path.to_path_buf()
}

#[cfg(windows)]
fn windows_command_path(path: &str) -> String {
    path.strip_prefix("\\\\?\\").unwrap_or(path).to_string()
}
