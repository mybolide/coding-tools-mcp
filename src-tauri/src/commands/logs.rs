use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

use serde::Serialize;
use tauri::State;

use crate::app_state::AppState;
use crate::error::{AppError, AppResult};
use crate::tunnel::log_dir_for_profile;
use crate::workspace::WorkspaceProfile;

const MAX_LOG_BYTES: usize = 8192;
const MAX_LOG_CHARS: usize = 4000;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogChunk {
    pub name: String,
    pub content: String,
}

fn profile_by_id(state: &AppState, id: &str) -> AppResult<WorkspaceProfile> {
    state.with_workspaces(|store| {
        store
            .get(id)
            .cloned()
            .ok_or_else(|| AppError::Message(format!("workspace not found: {id}")))
    })
}

fn log_file_names(profile: &WorkspaceProfile, service: &str) -> AppResult<Vec<&'static str>> {
    match service {
        "mcp" => {
            let mut names = vec!["stderr.log", "stdout.log"];
            if profile.tunnel.tunnel_type == "cloudflare" {
                names.insert(0, "cloudflared.log");
            }
            if profile.tunnel.tunnel_type == "frp" {
                names.insert(0, "frpc-mcp.log");
            }
            Ok(names)
        }
        "actions" => {
            let mut names = vec!["actions-stderr.log", "actions-stdout.log"];
            if profile.actions.tunnel_type == "cloudflare" {
                names.insert(0, "actions-cloudflared.log");
            }
            if profile.actions.tunnel_type == "frp" {
                names.insert(0, "frpc-actions.log");
            }
            Ok(names)
        }
        other => Err(AppError::Message(format!("unknown log service: {other}"))),
    }
}

fn read_log_tail(path: &Path) -> AppResult<String> {
    let mut file = File::open(path)?;
    let size = file.seek(SeekFrom::End(0))?;
    let start = size.saturating_sub(MAX_LOG_BYTES as u64);
    file.seek(SeekFrom::Start(start))?;
    let mut buf = Vec::new();
    file.read_to_end(&mut buf)?;
    let text = String::from_utf8_lossy(&buf).into_owned();
    Ok(if text.chars().count() > MAX_LOG_CHARS {
        text.chars()
            .rev()
            .take(MAX_LOG_CHARS)
            .collect::<String>()
            .chars()
            .rev()
            .collect()
    } else {
        text
    })
}

#[tauri::command]
pub async fn read_workspace_logs(
    state: State<'_, AppState>,
    id: String,
    service: String,
) -> AppResult<Vec<LogChunk>> {
    let profile = profile_by_id(&state, &id)?;
    let log_dir = log_dir_for_profile(&profile.id);
    let names = log_file_names(&profile, &service)?;

    let mut chunks = Vec::new();
    for name in names {
        let path = log_dir.join(name);
        if !path.exists() {
            continue;
        }
        let content = read_log_tail(&path)?;
        chunks.push(LogChunk {
            name: name.to_string(),
            content,
        });
    }

    Ok(chunks)
}
