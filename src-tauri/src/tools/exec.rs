use std::path::Path;
use std::time::{Duration, Instant};

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
        return Err(WorkspaceError::not_a_directory("workdir is not a directory"));
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
        )
        .await
    });

    match result {
        Ok(out) => Ok(tool_ok(out)),
        Err(e) => Err(e),
    }
}

async fn run_command(
    ctx: &ToolContext,
    cmd: &str,
    cwd: &Path,
    limit: Duration,
    yield_time: Duration,
    max_output: usize,
    tty: bool,
    stdin_text: &str,
) -> Result<Value, WorkspaceError> {
    let (program, args) = parse_and_resolve(cmd)?;
    let start = Instant::now();

    let mut command = Command::new(&program);
    command
        .args(&args)
        .current_dir(cwd)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());

    let child = command.spawn().map_err(|e| WorkspaceError::Tool {
        code: "COMMAND_REJECTED",
        message: format!("Failed to start command: {e}"),
        category: "runtime",
        retryable: false,
    })?;

    let session = ctx.sessions.insert(ExecSession::new(child));
    session.spawn_readers().await;

    if !tty {
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
    }

    let deadline = start + limit;
    loop {
        session.refresh_status().await;
        let exit_code = *session.exit_code.lock().expect("exit_code lock");
        if exit_code.is_some() {
            let snapshot = session.snapshot(max_output);
            ctx.sessions.remove(&session.session_id);
            return Ok(merge_exec_result(snapshot, start, false));
        }
        if !tty && Instant::now() >= deadline {
            session.kill_and_wait().await;
            session.refresh_status().await;
            ctx.sessions.remove(&session.session_id);
            return Err(WorkspaceError::Tool {
                code: "TIMEOUT",
                message: "Command timed out.".into(),
                category: "runtime",
                retryable: true,
            });
        }
        if Instant::now() - start >= yield_time || tty {
            let snapshot = session.snapshot(max_output);
            return Ok(merge_exec_result(snapshot, start, true));
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

fn merge_exec_result(mut snapshot: Value, start: Instant, keep_session: bool) -> Value {
    if let Some(obj) = snapshot.as_object_mut() {
        obj.insert("elapsed_ms".into(), json!(start.elapsed().as_millis()));
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

fn parse_and_resolve(cmd: &str) -> Result<(String, Vec<String>), WorkspaceError> {
    let parts = shell_words::split(cmd)
        .map_err(|_| WorkspaceError::invalid_argument("Invalid command syntax"))?;
    if parts.is_empty() {
        return Err(WorkspaceError::invalid_argument("Empty command"));
    }

    let program = resolve_program(&parts[0])?;
    Ok((program, parts[1..].to_vec()))
}

fn resolve_program(raw: &str) -> Result<String, WorkspaceError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(WorkspaceError::invalid_argument("Empty program"));
    }

    if trimmed.contains(['/', '\\']) {
        let path = Path::new(trimmed);
        if path.exists() {
            return Ok(path.to_string_lossy().into_owned());
        }
        return Err(WorkspaceError::Tool {
            code: "COMMAND_REJECTED",
            message: format!("Program not found: {trimmed}"),
            category: "runtime",
            retryable: false,
        });
    }

    which::which(trimmed).map(|p| p.to_string_lossy().into_owned()).map_err(|_| {
        WorkspaceError::Tool {
            code: "COMMAND_REJECTED",
            message: format!("Program not found on PATH: {trimmed}"),
            category: "runtime",
            retryable: false,
        }
    })
}
