use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use regex::Regex;
use serde_json::{json, Value};
use walkdir::WalkDir;

use crate::tools::workspace::{relative_display, tool_ok, Workspace, WorkspaceError};

pub fn read_file(ws: &Workspace, args: &Value) -> Result<Value, WorkspaceError> {
    let path = args
        .get("path")
        .and_then(Value::as_str)
        .ok_or_else(|| WorkspaceError::invalid_argument("path is required"))?;
    let resolved = ws.resolve_read_path(path)?;
    if resolved.path.is_dir() {
        return Err(WorkspaceError::Tool {
            code: "IS_DIRECTORY",
            message: "Path is a directory.".into(),
            category: "validation",
            retryable: false,
        });
    }
    let max_bytes = args
        .get("max_bytes")
        .and_then(Value::as_u64)
        .unwrap_or(131_072) as usize;
    let start_line = args
        .get("start_line")
        .and_then(Value::as_u64)
        .unwrap_or(1)
        .max(1) as usize;
    let end_line = args.get("end_line").and_then(Value::as_u64).map(|v| v as usize);

    let data = fs::read(&resolved.path).map_err(|_| WorkspaceError::not_found("File not found"))?;
    if data.iter().take(4096).any(|b| *b == 0) {
        return Err(WorkspaceError::Tool {
            code: "BINARY_FILE",
            message: "Binary file read blocked for text tool.".into(),
            category: "validation",
            retryable: false,
        });
    }
    let text = String::from_utf8(data).map_err(|_| WorkspaceError::Tool {
        code: "UNSUPPORTED_ENCODING",
        message: "File is not valid utf-8.".into(),
        category: "validation",
        retryable: false,
    })?;
    let lines: Vec<&str> = text.split_inclusive('\n').collect();
    let total_lines = lines.len();
    let end = end_line.unwrap_or(total_lines).min(total_lines);
    let selected: String = if end < start_line {
        String::new()
    } else {
        lines[(start_line - 1)..end].concat()
    };
    let (content, truncated, truncated_by) = truncate_bytes(&selected, max_bytes);
    let actual_end = if truncated && !content.is_empty() {
        start_line + content.lines().count().saturating_sub(1)
    } else {
        end
    };
    let mut warnings = Vec::new();
    if truncated {
        warnings.push("content truncated".to_string());
    }
    Ok(tool_ok(json!({
        "path": resolved.display,
        "content": content,
        "encoding": "utf-8",
        "start_line": start_line,
        "end_line": actual_end,
        "total_lines": total_lines,
        "total_bytes": text.len(),
        "bytes_read": content.len(),
        "truncated": truncated,
        "truncated_by": truncated_by,
        "warnings": warnings
    })))
}

pub fn list_dir(ws: &Workspace, args: &Value) -> Result<Value, WorkspaceError> {
    let path = args.get("path").and_then(Value::as_str).unwrap_or(".");
    let resolved = ws.resolve_read_path(path)?;
    if !resolved.path.is_dir() {
        return Err(WorkspaceError::not_a_directory("Path is not a directory"));
    }
    let recursive = args.get("recursive").and_then(Value::as_bool).unwrap_or(false);
    let max_depth = args
        .get("max_depth")
        .and_then(Value::as_u64)
        .unwrap_or(1)
        .max(1) as usize;
    let max_entries = args
        .get("max_entries")
        .and_then(Value::as_u64)
        .unwrap_or(1000) as usize;
    let include_hidden = args
        .get("include_hidden")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let include_ignored = args
        .get("include_ignored")
        .and_then(Value::as_bool)
        .unwrap_or(false);

    let mut entries = Vec::new();
    let mut truncated = false;
    collect_dir_entries(
        ws,
        &resolved.path,
        &resolved.display,
        1,
        max_depth,
        recursive,
        include_hidden,
        include_ignored,
        max_entries,
        &mut entries,
        &mut truncated,
    );
    entries.sort_by(|a, b| a["name"].as_str().cmp(&b["name"].as_str()));
    Ok(tool_ok(json!({
        "path": resolved.display,
        "entries": entries,
        "truncated": truncated,
        "warnings": if truncated { vec!["entry limit reached"] } else { vec![] }
    })))
}

pub fn list_files(ws: &Workspace, args: &Value) -> Result<Value, WorkspaceError> {
    let path = args.get("path").and_then(Value::as_str).unwrap_or(".");
    let resolved = ws.resolve_read_path(path)?;
    if !resolved.path.is_dir() {
        return Err(WorkspaceError::not_a_directory("Path is not a directory"));
    }
    let patterns = list_files_patterns(args);
    let exclude_patterns = string_list_arg(args, "exclude_patterns");
    let max_results = args
        .get("max_results")
        .and_then(Value::as_u64)
        .unwrap_or(5000) as usize;
    let include_hidden = args
        .get("include_hidden")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let include_ignored = args
        .get("include_ignored")
        .and_then(Value::as_bool)
        .unwrap_or(false);

    let mut files = Vec::new();
    let mut truncated = false;
    for entry in WalkDir::new(&resolved.path)
        .follow_links(false)
        .into_iter()
        .filter_map(Result::ok)
    {
        let p = entry.path();
        if p == resolved.path {
            continue;
        }
        if !ws.is_safe_read_path(p) {
            continue;
        }
        if ws.is_ignored_path(p, include_hidden, include_ignored) {
            if entry.file_type().is_dir() {
                continue;
            }
            continue;
        }
        if !entry.file_type().is_file() && !entry.file_type().is_symlink() {
            continue;
        }
        let rel = relative_display(ws.root(), p);
        if !patterns.iter().any(|pat| glob_match(pat, &rel)) {
            continue;
        }
        if exclude_patterns.iter().any(|pat| glob_match(pat, &rel)) {
            continue;
        }
        let meta = p.symlink_metadata().ok();
        files.push(json!({
            "path": rel,
            "type": if entry.file_type().is_symlink() { "symlink" } else { "file" },
            "size_bytes": meta.as_ref().map(|m| m.len()).unwrap_or(0),
            "modified": meta.and_then(|m| format_mtime(m.modified().ok()))
        }));
        if files.len() >= max_results {
            truncated = true;
            break;
        }
    }
    files.sort_by(|a, b| a["path"].as_str().cmp(&b["path"].as_str()));
    Ok(tool_ok(json!({
        "path": resolved.display,
        "files": files,
        "truncated": truncated,
        "warnings": if truncated { vec!["result limit reached"] } else { vec![] }
    })))
}

pub fn search_text(ws: &Workspace, args: &Value) -> Result<Value, WorkspaceError> {
    let query = args
        .get("query")
        .and_then(Value::as_str)
        .ok_or_else(|| WorkspaceError::invalid_argument("query is required"))?;
    let path = args.get("path").and_then(Value::as_str).unwrap_or(".");
    let resolved = ws.resolve_read_path(path)?;
    let use_regex = args.get("regex").and_then(Value::as_bool).unwrap_or(false);
    let case_sensitive = args
        .get("case_sensitive")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let max_results = args
        .get("max_results")
        .and_then(Value::as_u64)
        .unwrap_or(1000) as usize;
    let max_preview = args
        .get("max_preview_bytes")
        .and_then(Value::as_u64)
        .unwrap_or(512) as usize;

    let (include_globs, exclude_globs) = search_globs(args);
    let context_lines = args
        .get("context_lines")
        .and_then(Value::as_u64)
        .unwrap_or(0) as usize;
    let matcher = build_matcher(query, use_regex, case_sensitive)?;

    let file_paths: Vec<PathBuf> = if resolved.path.is_file() {
        vec![resolved.path.clone()]
    } else {
        WalkDir::new(&resolved.path)
            .follow_links(false)
            .into_iter()
            .filter_map(Result::ok)
            .filter(|e| e.file_type().is_file())
            .map(|e| e.path().to_path_buf())
            .collect()
    };

    let mut matches = Vec::new();
    let mut total = 0usize;
    for p in file_paths {
        if !ws.is_safe_read_path(&p) {
            continue;
        }
        if ws.is_ignored_path(&p, false, false) {
            continue;
        }
        let rel = relative_display(ws.root(), &p);
        if !passes_glob_filters(&rel, &include_globs, &exclude_globs) {
            continue;
        }
        let content = match fs::read_to_string(&p) {
            Ok(s) if !s.contains('\0') => s,
            _ => continue,
        };
        let lines: Vec<String> = content.lines().map(str::to_string).collect();
        for (idx, line) in lines.iter().enumerate() {
            if !matcher.is_match(line) {
                continue;
            }
            total += 1;
            if matches.len() >= max_results {
                continue;
            }
            let preview = if line.len() > max_preview {
                format!("{}...", &line[..max_preview])
            } else {
                line.clone()
            };
            let mut item = json!({
                "path": rel,
                "line": idx + 1,
                "column": 1,
                "preview": preview
            });
            if context_lines > 0 {
                let start = idx.saturating_sub(context_lines);
                let end = (idx + 1 + context_lines).min(lines.len());
                item["before"] = json!(lines[start..idx]
                    .iter()
                    .map(|s| s.as_str())
                    .collect::<Vec<_>>());
                item["after"] = json!(lines[idx + 1..end]
                    .iter()
                    .map(|s| s.as_str())
                    .collect::<Vec<_>>());
            }
            matches.push(item);
        }
    }
    Ok(tool_ok(json!({
        "query": query,
        "matches": matches,
        "total_matches": total,
        "truncated": total > matches.len(),
        "warnings": if total > matches.len() { vec!["result limit reached"] } else { vec![] }
    })))
}

fn build_matcher(
    query: &str,
    use_regex: bool,
    case_sensitive: bool,
) -> Result<Matcher, WorkspaceError> {
    if use_regex {
        let pattern = if case_sensitive {
            Regex::new(query)
        } else {
            Regex::new(&format!("(?i:{query})"))
        }
        .map_err(|e| WorkspaceError::invalid_argument(format!("Invalid regex: {e}")))?;
        Ok(Matcher::Regex(pattern))
    } else if case_sensitive {
        Ok(Matcher::Literal(query.to_string()))
    } else {
        Ok(Matcher::Literal(query.to_lowercase()))
    }
}

enum Matcher {
    Regex(Regex),
    Literal(String),
}

impl Matcher {
    fn is_match(&self, line: &str) -> bool {
        match self {
            Matcher::Regex(re) => re.is_match(line),
            Matcher::Literal(lit) => {
                if lit.chars().any(|c| c.is_uppercase()) {
                    line.contains(lit.as_str())
                } else {
                    line.to_lowercase().contains(lit)
                }
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn collect_dir_entries(
    ws: &Workspace,
    dir: &Path,
    display: &str,
    depth: usize,
    max_depth: usize,
    recursive: bool,
    include_hidden: bool,
    include_ignored: bool,
    max_entries: usize,
    entries: &mut Vec<Value>,
    truncated: &mut bool,
) {
    let read_dir = match fs::read_dir(dir) {
        Ok(rd) => rd,
        Err(_) => return,
    };
    for item in read_dir.flatten() {
        if *truncated {
            return;
        }
        let p = item.path();
        if ws.is_ignored_path(&p, include_hidden, include_ignored) {
            continue;
        }
        let name = item.file_name().to_string_lossy().into_owned();
        let rel = if display == "." {
            name.clone()
        } else {
            format!("{display}/{name}")
        };
        let ft = item.file_type().ok();
        let entry_type = if ft.as_ref().map(|t| t.is_symlink()).unwrap_or(false) {
            "symlink"
        } else if ft.as_ref().map(|t| t.is_dir()).unwrap_or(false) {
            "directory"
        } else if ft.as_ref().map(|t| t.is_file()).unwrap_or(false) {
            "file"
        } else {
            "other"
        };
        let meta = item.metadata().ok();
        entries.push(json!({
            "name": name,
            "path": rel.replace('\\', "/"),
            "type": entry_type,
            "size_bytes": meta.as_ref().map(|m| m.len()).unwrap_or(0),
            "modified": meta.and_then(|m| format_mtime(m.modified().ok())),
            "is_hidden": name.starts_with('.'),
            "is_ignored": false
        }));
        if entries.len() >= max_entries {
            *truncated = true;
            return;
        }
        if recursive && depth < max_depth && entry_type == "directory" && !p.is_symlink() {
            collect_dir_entries(
                ws,
                &p,
                &rel.replace('\\', "/"),
                depth + 1,
                max_depth,
                recursive,
                include_hidden,
                include_ignored,
                max_entries,
                entries,
                truncated,
            );
        }
    }
}

fn truncate_bytes(text: &str, max_bytes: usize) -> (String, bool, Option<&'static str>) {
    let bytes = text.as_bytes();
    if bytes.len() <= max_bytes {
        return (text.to_string(), false, None);
    }
    let mut end = max_bytes;
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    (
        text[..end].to_string(),
        true,
        Some("bytes"),
    )
}

fn string_list_arg(args: &Value, key: &str) -> Vec<String> {
    args.get(key)
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

fn list_files_patterns(args: &Value) -> Vec<String> {
    let patterns = string_list_arg(args, "patterns");
    if !patterns.is_empty() {
        return patterns;
    }
    if let Some(glob) = args.get("glob").and_then(Value::as_str) {
        if !glob.is_empty() {
            return vec![glob.to_string()];
        }
    }
    vec!["**/*".to_string()]
}

fn search_globs(args: &Value) -> (Vec<String>, Vec<String>) {
    let mut include = string_list_arg(args, "include_globs");
    if let Some(glob) = args.get("glob").and_then(Value::as_str) {
        if !glob.is_empty() {
            include.push(glob.to_string());
        }
    }
    (include, string_list_arg(args, "exclude_globs"))
}

fn passes_glob_filters(rel: &str, include: &[String], exclude: &[String]) -> bool {
    if !include.is_empty() && !include.iter().any(|pat| glob_match(pat, rel)) {
        return false;
    }
    !exclude.iter().any(|pat| glob_match(pat, rel))
}

fn glob_match(pattern: &str, path: &str) -> bool {
    let pat = pattern.replace('\\', "/");
    let p = path.replace('\\', "/");
    if pat == "**/*" || pat == "*" {
        return true;
    }
    if let Some(suffix) = pat.strip_prefix("**/") {
        return simple_glob(suffix, &p) || p.split('/').any(|part| simple_glob(suffix, part));
    }
    simple_glob(&pat, &p)
}

fn simple_glob(pattern: &str, text: &str) -> bool {
    glob::Pattern::new(pattern)
        .map(|p| p.matches(text))
        .unwrap_or(false)
}

fn format_mtime(st: Option<SystemTime>) -> Option<String> {
    st.map(|t| {
        let d = t
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default();
        format!("{}.{:03}Z", d.as_secs(), d.subsec_millis())
    })
}
