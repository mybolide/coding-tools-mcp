use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use serde_json::{json, Value};

use crate::tools::workspace::{tool_ok, Workspace, WorkspaceError};

pub fn apply_patch(ws: &Workspace, args: &Value) -> Result<Value, WorkspaceError> {
    let patch = args
        .get("patch")
        .and_then(Value::as_str)
        .ok_or_else(|| WorkspaceError::invalid_argument("patch is required"))?;
    let dry_run = args.get("dry_run").and_then(Value::as_bool).unwrap_or(false);

    let file_patches = parse_unified_diff(patch)?;
    if file_patches.is_empty() {
        return Err(patch_failed("No files were modified."));
    }

    let mut affected = Vec::new();
    let mut summaries = Vec::new();
    let mut staged: HashMap<String, Option<String>> = HashMap::new();

    for fp in &file_patches {
        ws.reject_unsafe_text(&fp.path)?;
        let resolved = if fp.is_new_file {
            ws.resolve_for_write(&fp.path)?
        } else {
            ws.resolve_existing(&fp.path)?
        };
        ws.reject_write_symlink(&fp.path)?;

        let original = if resolved.existed {
            fs::read_to_string(&resolved.path).map_err(|_| {
                WorkspaceError::not_found(format!("File not found: {}", fp.path))
            })?
        } else if fp.is_new_file || fp.is_deleted {
            String::new()
        } else {
            return Err(patch_failed(format!("File not found: {}", fp.path)));
        };

        if fp.is_deleted {
            staged.insert(resolved.display.clone(), None);
            affected.push(json!({ "path": resolved.display, "operation": "delete" }));
            summaries.push(format!("D {}", resolved.display));
            continue;
        }

        let updated = apply_hunks(&original, &fp.hunks)?;
        let op = if fp.is_new_file || !resolved.existed {
            "add"
        } else {
            "update"
        };
        staged.insert(resolved.display.clone(), Some(updated));
        affected.push(json!({ "path": resolved.display, "operation": op }));
        summaries.push(format!(
            "{} {}",
            if op == "add" { "A" } else { "M" },
            resolved.display
        ));
    }

    if !dry_run {
        commit_staged(ws, &staged)?;
    }

    Ok(tool_ok(json!({
        "dry_run": dry_run,
        "clean": true,
        "summary": summaries.join("\n"),
        "affected_files": affected,
        "warnings": []
    })))
}

#[derive(Debug)]
struct FilePatch {
    path: String,
    hunks: Vec<Hunk>,
    is_new_file: bool,
    is_deleted: bool,
}

#[derive(Debug)]
struct Hunk {
    lines: Vec<HunkLine>,
}

#[derive(Debug)]
enum HunkLine {
    Context(String),
    Add(String),
    Remove(String),
}

fn parse_unified_diff(patch: &str) -> Result<Vec<FilePatch>, WorkspaceError> {
    let mut files = Vec::new();
    let mut current: Option<FilePatch> = None;
    let mut current_hunk: Option<Hunk> = None;

    for line in patch.lines() {
        if line.starts_with("--- ") {
            if let Some(h) = current_hunk.take() {
                if let Some(ref mut f) = current {
                    f.hunks.push(h);
                }
            }
            if let Some(f) = current.take() {
                files.push(f);
            }
            let path = parse_diff_path(line.strip_prefix("--- ").unwrap_or(""));
            current = Some(FilePatch {
                path,
                hunks: Vec::new(),
                is_new_file: false,
                is_deleted: false,
            });
        } else if line.starts_with("+++ ") {
            if let Some(ref mut f) = current {
                let new_path = parse_diff_path(line.strip_prefix("+++ ").unwrap_or(""));
                if !new_path.is_empty() && new_path != "/dev/null" {
                    f.path = new_path;
                }
                if line.contains("/dev/null") {
                    f.is_deleted = true;
                }
            }
        } else if line.starts_with("@@") {
            if let Some(h) = current_hunk.take() {
                if let Some(ref mut f) = current {
                    f.hunks.push(h);
                }
            }
            current_hunk = Some(Hunk { lines: Vec::new() });
            if let Some(ref mut f) = current {
                if f.hunks.is_empty() && !f.is_deleted {
                    f.is_new_file = true;
                }
            }
        } else if let Some(ref mut hunk) = current_hunk {
            if let Some(rest) = line.strip_prefix('+') {
                hunk.lines.push(HunkLine::Add(rest.to_string()));
            } else if let Some(rest) = line.strip_prefix('-') {
                hunk.lines.push(HunkLine::Remove(rest.to_string()));
            } else if let Some(rest) = line.strip_prefix(' ') {
                hunk.lines.push(HunkLine::Context(rest.to_string()));
            } else if line.is_empty() {
                hunk.lines.push(HunkLine::Context(String::new()));
            }
        }
    }
    if let Some(h) = current_hunk.take() {
        if let Some(ref mut f) = current {
            f.hunks.push(h);
        }
    }
    if let Some(f) = current.take() {
        files.push(f);
    }
    Ok(files)
}

fn parse_diff_path(raw: &str) -> String {
    let trimmed = raw.trim();
    let path = trimmed
        .strip_prefix("a/")
        .or_else(|| trimmed.strip_prefix("b/"))
        .unwrap_or(trimmed);
    if path == "/dev/null" {
        return String::new();
    }
    path.replace('\\', "/")
}

fn apply_hunks(original: &str, hunks: &[Hunk]) -> Result<String, WorkspaceError> {
    let mut lines: Vec<String> = if original.is_empty() {
        Vec::new()
    } else {
        original.split_inclusive('\n').map(str::to_string).collect()
    };
    let mut offset: i64 = 0;

    for hunk in hunks {
        let search_at = 0usize;
        let hunk_old: Vec<String> = hunk
            .lines
            .iter()
            .filter_map(|l| match l {
                HunkLine::Context(s) | HunkLine::Remove(s) => Some(s.clone()),
                HunkLine::Add(_) => None,
            })
            .collect();

        let pos = find_hunk_position(&lines, &hunk_old, search_at)
            .ok_or_else(|| patch_failed("Hunk context did not match file content."))?;

        let mut idx = pos;
        for hl in &hunk.lines {
            match hl {
                HunkLine::Context(_) => idx += 1,
                HunkLine::Remove(_) => {
                    if idx < lines.len() {
                        lines.remove(idx);
                    }
                }
                HunkLine::Add(s) => {
                    let mut line = s.clone();
                    if !line.ends_with('\n') && idx < lines.len() {
                        // preserve newline style
                    }
                    if idx == lines.len() && !line.ends_with('\n') {
                        line.push('\n');
                    }
                    lines.insert(idx, line);
                    idx += 1;
                }
            }
        }
        offset += 0; // reserved for future fuzzy offset
        let _ = offset;
    }
    Ok(lines.concat())
}

fn find_hunk_position(lines: &[String], pattern: &[String], start: usize) -> Option<usize> {
    if pattern.is_empty() {
        return Some(start);
    }
    for i in start..=lines.len().saturating_sub(pattern.len()) {
        if lines[i..i + pattern.len()]
            .iter()
            .zip(pattern.iter())
            .all(|(a, b)| a.trim_end_matches('\n') == b.trim_end_matches('\n'))
        {
            return Some(i);
        }
    }
    None
}

fn commit_staged(ws: &Workspace, staged: &HashMap<String, Option<String>>) -> Result<(), WorkspaceError> {
    let mut backups: HashMap<PathBuf, Option<Vec<u8>>> = HashMap::new();
    for (rel, content) in staged {
        let resolved = if content.is_none() {
            ws.resolve_existing(rel)?
        } else {
            ws.resolve_for_write(rel)?
        };
        let path = resolved.path.clone();
        backups.insert(
            path.clone(),
            if path.exists() && path.is_file() {
                Some(fs::read(&path).unwrap_or_default())
            } else {
                None
            },
        );
        if let Err(err) = (|| -> Result<(), std::io::Error> {
            if let Some(text) = content {
                if let Some(parent) = path.parent() {
                    fs::create_dir_all(parent)?;
                }
                fs::write(&path, text)?;
            } else if path.exists() && path.is_file() {
                fs::remove_file(&path)?;
            }
            Ok(())
        })() {
            restore_backups(&backups);
            return Err(patch_failed(format!("Failed to write file: {err}")));
        }
    }
    Ok(())
}

fn restore_backups(backups: &HashMap<PathBuf, Option<Vec<u8>>>) {
    for (path, data) in backups {
        match data {
            None => {
                let _ = fs::remove_file(path);
            }
            Some(bytes) => {
                if let Some(parent) = path.parent() {
                    let _ = fs::create_dir_all(parent);
                }
                let _ = fs::write(path, bytes);
            }
        }
    }
}

fn patch_failed(message: impl Into<String>) -> WorkspaceError {
    WorkspaceError::Tool {
        code: "PATCH_FAILED",
        message: message.into(),
        category: "validation",
        retryable: false,
    }
}
