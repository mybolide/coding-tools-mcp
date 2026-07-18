use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use fs2::FileExt;
use sha2::{Digest, Sha256};

use crate::tools::workspace::{relative_display, Workspace, WorkspaceError, WorkspaceResult};

use super::markdown;
use super::model::{HistoryDocument, HistoryIndex, IndexEntry, ScanReport};

pub const DEFAULT_HISTORY_DIR: &str = "docs/history-session";

pub struct HistoryLock {
    file: File,
}

impl Drop for HistoryLock {
    fn drop(&mut self) {
        let _ = FileExt::unlock(&self.file);
    }
}

pub fn resolve_history_dir(
    workspace: &Workspace,
    workspace_root: Option<&str>,
    history_dir: Option<&str>,
) -> WorkspaceResult<PathBuf> {
    if let Some(requested_root) = workspace_root {
        let requested_path = Path::new(requested_root.trim());
        let candidate = if requested_path.is_absolute() {
            requested_path.to_path_buf()
        } else {
            workspace.root().join(requested_path)
        };
        let requested = candidate
            .canonicalize()
            .map_err(|_| WorkspaceError::invalid_argument("workspace_root does not exist"))?;
        if requested != workspace.root() {
            return Err(WorkspaceError::path_outside_workspace());
        }
    }

    let raw = history_dir.unwrap_or(DEFAULT_HISTORY_DIR).trim();
    if raw.is_empty() || workspace.reject_unsafe_text(raw).is_err() {
        return Err(WorkspaceError::path_outside_workspace());
    }
    let candidate = workspace
        .root()
        .join(raw.replace('/', std::path::MAIN_SEPARATOR_STR));
    ensure_safe_candidate(workspace, &candidate)?;
    if candidate.exists() && !candidate.is_dir() {
        return Err(WorkspaceError::not_a_directory(
            "history_dir must be a directory",
        ));
    }
    Ok(candidate)
}

fn ensure_safe_candidate(workspace: &Workspace, candidate: &Path) -> WorkspaceResult<()> {
    if candidate.exists() || candidate.is_symlink() {
        let resolved = candidate
            .canonicalize()
            .map_err(|_| WorkspaceError::path_outside_workspace())?;
        if !resolved.starts_with(workspace.root()) {
            return Err(WorkspaceError::path_outside_workspace());
        }
        return Ok(());
    }
    let mut ancestor = candidate.parent();
    while let Some(path) = ancestor {
        if path.exists() || path.is_symlink() {
            let resolved = path
                .canonicalize()
                .map_err(|_| WorkspaceError::path_outside_workspace())?;
            if !resolved.starts_with(workspace.root()) {
                return Err(WorkspaceError::path_outside_workspace());
            }
            return Ok(());
        }
        ancestor = path.parent();
    }
    Err(WorkspaceError::path_outside_workspace())
}

pub fn ensure_directory(path: &Path) -> WorkspaceResult<()> {
    fs::create_dir_all(path).map_err(|error| io_error("HISTORY_WRITE_FAILED", error, true))
}

pub fn lock_directory(path: &Path) -> WorkspaceResult<HistoryLock> {
    ensure_directory(path)?;
    let lock_path = path.join(".history.lock");
    let file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(lock_path)
        .map_err(|error| io_error("HISTORY_LOCK_FAILED", error, true))?;
    FileExt::lock_exclusive(&file).map_err(|error| io_error("HISTORY_LOCK_FAILED", error, true))?;
    Ok(HistoryLock { file })
}

pub fn scan(workspace: &Workspace, history_dir: &Path) -> WorkspaceResult<ScanReport> {
    if !history_dir.exists() {
        return Ok(ScanReport::default());
    }
    ensure_safe_candidate(workspace, history_dir)?;
    let mut report = ScanReport::default();
    let entries =
        fs::read_dir(history_dir).map_err(|error| io_error("HISTORY_READ_FAILED", error, true))?;
    for entry in entries {
        let entry = entry.map_err(|error| io_error("HISTORY_READ_FAILED", error, true))?;
        let file_type = entry
            .file_type()
            .map_err(|error| io_error("HISTORY_READ_FAILED", error, true))?;
        if !file_type.is_file() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        if matches!(name.as_str(), "README.md" | "index.json" | ".history.lock")
            || name.starts_with(".history-tmp-")
        {
            continue;
        }
        let path = entry.path();
        let Some(stem) = path.file_stem().and_then(|value| value.to_str()) else {
            report.invalid_files.push(name);
            continue;
        };
        let is_markdown = path.extension().and_then(|value| value.to_str()) == Some("md");
        let number = stem.parse::<u64>().ok();
        if !is_markdown
            || number.is_none()
            || number == Some(0)
            || number.map(|value| value.to_string()) != Some(stem.to_string())
        {
            report.invalid_files.push(name);
            continue;
        }
        let number = number.expect("validated number");
        let bytes =
            fs::read(&path).map_err(|error| io_error("HISTORY_READ_FAILED", error, true))?;
        let content = String::from_utf8(bytes).map_err(|error| WorkspaceError::ToolDetails {
            code: "HISTORY_INVALID_UTF8",
            message: "History Markdown must be UTF-8.".into(),
            category: "validation",
            retryable: false,
            details: serde_json::json!({"file": name, "error": error.to_string()}),
        })?;
        if content.trim().is_empty() {
            report.empty_files.push(name.clone());
        }
        report.documents.push(HistoryDocument {
            number,
            path: relative_display(workspace.root(), &path),
            session_key: markdown::metadata(&content, "Session key"),
            created_at: markdown::metadata(&content, "Created"),
            updated_at: markdown::metadata(&content, "Updated"),
            content,
        });
    }
    report.documents.sort_by_key(|document| document.number);
    report.invalid_files.sort();
    report.empty_files.sort();
    report.numbers = report
        .documents
        .iter()
        .map(|document| document.number)
        .collect();
    if let Some(latest) = report.latest_number() {
        let present = report.numbers.iter().copied().collect::<BTreeSet<_>>();
        report.missing_numbers = (1..=latest)
            .filter(|number| !present.contains(number))
            .collect();
    }
    let mut keys = BTreeMap::<String, usize>::new();
    for key in report
        .documents
        .iter()
        .filter_map(|document| document.session_key.as_ref())
    {
        *keys.entry(key.clone()).or_default() += 1;
    }
    report.duplicate_session_keys = keys
        .into_iter()
        .filter_map(|(key, count)| (count > 1).then_some(key))
        .collect();
    Ok(report)
}

pub fn rebuild_index(report: &ScanReport) -> HistoryIndex {
    let duplicates = report
        .duplicate_session_keys
        .iter()
        .collect::<BTreeSet<_>>();
    let mut index = HistoryIndex {
        latest_number: report.latest_number().unwrap_or(0),
        ..HistoryIndex::default()
    };
    for document in &report.documents {
        let Some(session_key) = document.session_key.as_ref() else {
            continue;
        };
        if duplicates.contains(session_key) {
            continue;
        }
        index.sessions.insert(
            session_key.clone(),
            IndexEntry {
                number: document.number,
                path: document.path.clone(),
                created_at: document.created_at.clone().unwrap_or_default(),
                updated_at: document.updated_at.clone().unwrap_or_default(),
            },
        );
    }
    index
}

pub fn read_index(history_dir: &Path) -> WorkspaceResult<Option<HistoryIndex>> {
    let path = history_dir.join("index.json");
    if !path.exists() {
        return Ok(None);
    }
    let content =
        fs::read_to_string(&path).map_err(|error| io_error("HISTORY_READ_FAILED", error, true))?;
    serde_json::from_str(&content)
        .map(Some)
        .map_err(|error| WorkspaceError::ToolDetails {
            code: "HISTORY_INDEX_INVALID",
            message: "History index is not valid JSON.".into(),
            category: "validation",
            retryable: true,
            details: serde_json::json!({"error": error.to_string()}),
        })
}

pub fn write_index(history_dir: &Path, index: &HistoryIndex) -> WorkspaceResult<()> {
    let content =
        serde_json::to_vec_pretty(index).map_err(|error| WorkspaceError::ToolDetails {
            code: "HISTORY_WRITE_FAILED",
            message: "Unable to serialize history index.".into(),
            category: "internal",
            retryable: true,
            details: serde_json::json!({"error": error.to_string()}),
        })?;
    atomic_write(&history_dir.join("index.json"), &content)
}

pub fn write_markdown(path: &Path, content: &str) -> WorkspaceResult<()> {
    atomic_write(path, content.as_bytes())
}

pub fn sha256(content: &[u8]) -> String {
    format!("{:x}", Sha256::digest(content))
}

fn atomic_write(target: &Path, content: &[u8]) -> WorkspaceResult<()> {
    let parent = target
        .parent()
        .ok_or_else(|| WorkspaceError::invalid_argument("History target has no parent"))?;
    ensure_directory(parent)?;
    let temp = parent.join(format!(".history-tmp-{}", uuid::Uuid::new_v4()));
    let result = (|| -> io::Result<()> {
        let mut file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temp)?;
        file.write_all(content)?;
        file.sync_all()?;
        atomic_replace(&temp, target)?;
        if let Ok(directory) = File::open(parent) {
            let _ = directory.sync_all();
        }
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    result.map_err(|error| io_error("HISTORY_WRITE_FAILED", error, true))
}

#[cfg(not(windows))]
fn atomic_replace(source: &Path, target: &Path) -> io::Result<()> {
    fs::rename(source, target)
}

#[cfg(windows)]
fn atomic_replace(source: &Path, target: &Path) -> io::Result<()> {
    use std::os::windows::ffi::OsStrExt;

    use windows::core::PCWSTR;
    use windows::Win32::Storage::FileSystem::{
        MoveFileExW, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH,
    };

    let source = source
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect::<Vec<_>>();
    let target = target
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect::<Vec<_>>();
    unsafe {
        MoveFileExW(
            PCWSTR(source.as_ptr()),
            PCWSTR(target.as_ptr()),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
        .map_err(|error| io::Error::other(error.to_string()))
    }
}

fn io_error(code: &'static str, error: io::Error, retryable: bool) -> WorkspaceError {
    WorkspaceError::ToolDetails {
        code,
        message: error.to_string(),
        category: "filesystem",
        retryable,
        details: serde_json::json!({"kind": format!("{:?}", error.kind())}),
    }
}
