mod markdown;
mod model;
mod storage;

use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use crate::tools::context::ToolContext;
use crate::tools::workspace::{tool_ok, WorkspaceError, WorkspaceResult};

pub fn bootstrap(ctx: &ToolContext, args: &Value) -> WorkspaceResult<Value> {
    let (session_key, source) = resolve_session_key(args)?;
    let history_dir = resolve_dir(ctx, args)?;
    storage::ensure_directory(&history_dir)?;
    let _lock = storage::lock_directory(&history_dir)?;
    let report = storage::scan(&ctx.workspace, &history_dir)?;
    reject_ambiguous_history(&report)?;
    if !report.missing_numbers.is_empty() {
        return Err(history_error(
            "HISTORY_SEQUENCE_CONFLICT",
            "History numbering contains gaps; run history_session_validate before creating a session.",
            "validation",
            true,
            json!({"missing_numbers": report.missing_numbers}),
        ));
    }

    let mut warnings = Vec::<String>::new();
    match storage::read_index(&history_dir) {
        Ok(Some(_)) => {}
        Ok(None) => warnings.push("历史索引缺失，已根据 Markdown 重建。".into()),
        Err(_) => warnings.push("历史索引损坏，已根据 Markdown 重建。".into()),
    }
    let readme = history_dir.join("README.md");
    if readme.exists() {
        fs::read_to_string(&readme).map_err(|error| {
            history_error(
                "HISTORY_READ_FAILED",
                &error.to_string(),
                "filesystem",
                true,
                json!({"path": "docs/history-session/README.md"}),
            )
        })?;
    } else {
        warnings.push("docs/history-session/README.md 不存在。".into());
    }

    let existing = report
        .documents
        .iter()
        .find(|document| document.session_key.as_deref() == Some(session_key.as_str()));
    let (current_number, current_path, created, resumed) = if let Some(document) = existing {
        (document.number, document.path.clone(), false, true)
    } else {
        if !args
            .get("create_if_missing")
            .and_then(Value::as_bool)
            .unwrap_or(true)
        {
            return Err(history_error(
                "SESSION_NOT_BOOTSTRAPPED",
                "No history mapping exists for this session_key.",
                "not_found",
                false,
                json!({"session_key_source": source}),
            ));
        }
        let number = report.latest_number().unwrap_or(0) + 1;
        let relative_path = format!("{}/{number}.md", history_dir_display(ctx, &history_dir));
        let timestamp = now_timestamp();
        let title = args
            .get("title")
            .and_then(Value::as_str)
            .unwrap_or("开发会话");
        let content = markdown::render_document(
            number,
            title,
            &session_key,
            &timestamp,
            &timestamp,
            "active",
            &[],
        );
        storage::write_markdown(&history_dir.join(format!("{number}.md")), &content)?;
        (number, relative_path, true, false)
    };

    let refreshed = storage::scan(&ctx.workspace, &history_dir)?;
    reject_ambiguous_history(&refreshed)?;
    storage::write_index(&history_dir, &storage::rebuild_index(&refreshed))?;

    let prior = report
        .documents
        .iter()
        .filter(|document| document.number != current_number)
        .collect::<Vec<_>>();
    let history_numbers = prior
        .iter()
        .map(|document| document.number)
        .collect::<Vec<_>>();
    let session_summaries = prior
        .iter()
        .map(|document| {
            json!({
                "number": document.number,
                "path": document.path,
                "summary": markdown::summary(&document.content)
            })
        })
        .collect::<Vec<_>>();
    let all_history_summary = session_summaries
        .iter()
        .map(|summary| {
            format!(
                "会话 {}（{}）：{}",
                summary["number"].as_u64().unwrap_or_default(),
                summary["path"].as_str().unwrap_or_default(),
                summary["summary"].as_str().unwrap_or_default()
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    let latest = prior.iter().max_by_key(|document| document.number).copied();
    let mut digest = Sha256::new();
    let mut total_bytes = 0_u64;
    for document in &prior {
        digest.update(document.number.to_le_bytes());
        digest.update(document.content.as_bytes());
        total_bytes += document.content.len() as u64;
    }

    Ok(tool_ok(json!({
        "is_new_session": created,
        "session_key_source": source,
        "history_numbers": history_numbers,
        "history_count": prior.len(),
        "latest_completed_number": latest.map(|document| document.number),
        "latest_completed_path": latest.map(|document| document.path.clone()),
        "current_number": current_number,
        "current_path": current_path,
        "created": created,
        "resumed": resumed,
        "sequence_valid": report.sequence_valid(),
        "all_history_summary": all_history_summary,
        "session_summaries": session_summaries,
        "latest_handoff": latest.map(|document| document.content.clone()),
        "history_read_mode": "all_summaries_plus_latest_full",
        "total_history_bytes": total_bytes,
        "full_history_included": false,
        "history_digest": format!("{:x}", digest.finalize()),
        "persistence_mode": "model_mediated_tool_calls",
        "assistant_instructions": "Read all_history_summary and latest_handoff before continuing the project. Use history_session_checkpoint when the user or active workflow requests a persisted handoff. Only state that progress was persisted after checkpoint returns ok=true.",
        "required_next_actions": [
            "read_all_history_summary",
            "read_latest_handoff",
            "verify_workspace_state",
            "execute_user_task"
        ],
        "checkpoint_policy": {
            "tool": "history_session_checkpoint",
            "required_before_final_response": false,
            "applies_after_bootstrap": true,
            "automatic_background_persistence": false
        },
        "warnings": warnings
    })))
}

pub fn checkpoint(ctx: &ToolContext, args: &Value) -> WorkspaceResult<Value> {
    let (session_key, _) = resolve_session_key(args)?;
    let history_dir = resolve_dir(ctx, args)?;
    if !history_dir.exists() {
        return Err(session_not_bootstrapped());
    }
    let _lock = storage::lock_directory(&history_dir)?;
    let report = storage::scan(&ctx.workspace, &history_dir)?;
    reject_ambiguous_history(&report)?;
    let document = report
        .documents
        .iter()
        .find(|document| document.session_key.as_deref() == Some(session_key.as_str()))
        .ok_or_else(session_not_bootstrapped)?;

    let timestamp = now_timestamp();
    let mut record = markdown::checkpoint_from_args(args, &timestamp)
        .map_err(WorkspaceError::invalid_argument)?;
    let redacted = markdown::redact_record(&mut record);
    let mut records = markdown::parse_checkpoint_records(&document.content);
    let mut duplicate_ignored = false;
    let mut updated = false;
    if let Some(existing) = records
        .iter_mut()
        .find(|existing| existing.turn_id == record.turn_id)
    {
        if existing == &record {
            duplicate_ignored = true;
        } else {
            *existing = record.clone();
            updated = true;
        }
    } else {
        records.push(record.clone());
        updated = true;
    }

    let final_content = if duplicate_ignored {
        document.content.clone()
    } else {
        let created_at = document
            .created_at
            .clone()
            .unwrap_or_else(|| timestamp.clone());
        markdown::render_document(
            document.number,
            &markdown::document_title(&document.content, document.number),
            &session_key,
            &created_at,
            &record.timestamp,
            "active",
            &records,
        )
    };
    if !duplicate_ignored {
        storage::write_markdown(
            &history_dir.join(format!("{}.md", document.number)),
            &final_content,
        )?;
        let refreshed = storage::scan(&ctx.workspace, &history_dir)?;
        storage::write_index(&history_dir, &storage::rebuild_index(&refreshed))?;
    }
    let warnings = if redacted {
        vec!["检测到疑似敏感信息，归档内容已脱敏。"]
    } else {
        Vec::new()
    };
    Ok(tool_ok(json!({
        "session_number": document.number,
        "path": document.path,
        "turn_id": record.turn_id,
        "created": false,
        "updated": updated,
        "duplicate_ignored": duplicate_ignored,
        "content_hash": storage::sha256(final_content.as_bytes()),
        "warnings": warnings
    })))
}

pub fn validate(ctx: &ToolContext, args: &Value) -> WorkspaceResult<Value> {
    let history_dir = resolve_dir(ctx, args)?;
    let repair = args.get("repair").and_then(Value::as_bool).unwrap_or(false);
    if repair {
        storage::ensure_directory(&history_dir)?;
    }
    let mut index_status = "missing";
    if history_dir.exists() {
        index_status = match storage::read_index(&history_dir) {
            Ok(Some(_)) => "valid",
            Ok(None) => "missing",
            Err(_) => "invalid",
        };
    }
    let report = storage::scan(&ctx.workspace, &history_dir)?;
    let mut warnings = Vec::<String>::new();
    if !report.duplicate_session_keys.is_empty() {
        warnings.push("存在重复 session_key，相关映射未写入索引。".into());
    }
    let repaired = if repair {
        let _lock = storage::lock_directory(&history_dir)?;
        let locked_report = storage::scan(&ctx.workspace, &history_dir)?;
        storage::write_index(&history_dir, &storage::rebuild_index(&locked_report))?;
        true
    } else {
        false
    };
    let latest_number = report.latest_number();
    let latest_path = latest_number.and_then(|number| {
        report
            .documents
            .iter()
            .find(|document| document.number == number)
            .map(|document| document.path.clone())
    });
    Ok(tool_ok(json!({
        "sequence_valid": report.sequence_valid(),
        "numbers": report.numbers,
        "missing_numbers": report.missing_numbers,
        "duplicate_session_keys": report.duplicate_session_keys,
        "invalid_files": report.invalid_files,
        "empty_files": report.empty_files,
        "latest_number": latest_number,
        "latest_path": latest_path,
        "index_status": index_status,
        "repaired": repaired,
        "warnings": warnings
    })))
}

fn resolve_dir(ctx: &ToolContext, args: &Value) -> WorkspaceResult<std::path::PathBuf> {
    storage::resolve_history_dir(
        &ctx.workspace,
        args.get("workspace_root").and_then(Value::as_str),
        args.get("history_dir").and_then(Value::as_str),
    )
}

fn resolve_session_key(args: &Value) -> WorkspaceResult<(String, &'static str)> {
    if let Some(value) = args
        .get("_host_session_key")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        return Ok((value.to_string(), "platform_conversation_id"));
    }
    if let Some(value) = args
        .get("session_key")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        return Ok((value.to_string(), "explicit_session_key"));
    }
    Err(history_error(
        "SESSION_ID_UNAVAILABLE",
        "A stable ChatGPT session identifier is required.",
        "validation",
        false,
        json!({}),
    ))
}

fn reject_ambiguous_history(report: &model::ScanReport) -> WorkspaceResult<()> {
    if report.duplicate_session_keys.is_empty() {
        return Ok(());
    }
    Err(history_error(
        "HISTORY_INDEX_CONFLICT",
        "Multiple history files declare the same session_key.",
        "validation",
        false,
        json!({"duplicate_session_keys": report.duplicate_session_keys}),
    ))
}

fn session_not_bootstrapped() -> WorkspaceError {
    history_error(
        "SESSION_NOT_BOOTSTRAPPED",
        "The session_key has not been bootstrapped.",
        "not_found",
        false,
        json!({}),
    )
}

fn history_error(
    code: &'static str,
    message: &str,
    category: &'static str,
    retryable: bool,
    details: Value,
) -> WorkspaceError {
    WorkspaceError::ToolDetails {
        code,
        message: message.into(),
        category,
        retryable,
        details,
    }
}

fn history_dir_display(ctx: &ToolContext, path: &std::path::Path) -> String {
    crate::tools::workspace::relative_display(ctx.workspace.root(), path)
}

fn now_timestamp() -> String {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    format!("unix:{seconds}")
}
