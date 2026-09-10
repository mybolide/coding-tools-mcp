use std::collections::VecDeque;
use std::fs::{self, File};
use std::io::{BufRead, BufReader, Read};
use std::path::Path;
use std::time::SystemTime;

use regex::Regex;
use serde_json::{json, Value};
use walkdir::WalkDir;

use crate::tools::workspace::{relative_display, tool_ok, Workspace, WorkspaceError};

/// Default per-file cap for `search_text` to avoid loading multi-GB assets.
const DEFAULT_SEARCH_MAX_FILE_BYTES: u64 = 2 * 1024 * 1024;
const BINARY_PEEK_BYTES: usize = 8192;

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

    let file_len = fs::metadata(&resolved.path)
        .map_err(|_| WorkspaceError::not_found("File not found"))?
        .len();
    if file_len > STREAMING_READ_THRESHOLD {
        return read_file_streaming(&resolved.path, &resolved.display, max_bytes, start_line, end_line);
    }
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

/// Files above this size take the bounded-memory streaming path: output
/// semantics are identical to the in-memory path, but peak memory is the
/// 64 KiB read buffer plus at most `max_bytes` of selected content instead
/// of two full copies of the file plus a whole-file line index.
const STREAMING_READ_THRESHOLD: u64 = 4 * 1024 * 1024;

#[allow(clippy::too_many_arguments)]
fn read_file_streaming(
    path: &Path,
    display: &str,
    max_bytes: usize,
    start_line: usize,
    end_line: Option<usize>,
) -> Result<Value, WorkspaceError> {
    let unsupported_encoding = || WorkspaceError::Tool {
        code: "UNSUPPORTED_ENCODING",
        message: "File is not valid utf-8.".into(),
        category: "validation",
        retryable: false,
    };
    let not_found = || WorkspaceError::not_found("File not found");

    let mut file = File::open(path).map_err(|_| not_found())?;
    let mut buf = vec![0u8; 64 * 1024];
    let mut utf8_carry: Vec<u8> = Vec::new();
    let mut seen: usize = 0;
    let mut last_byte: u8 = 0;
    let mut total_lines: usize = 0;
    let mut selected: Vec<u8> = Vec::new();
    let mut line_pending: Vec<u8> = Vec::new();
    let mut overflow = false;

    loop {
        let read = file.read(&mut buf).map_err(|_| not_found())?;
        if read == 0 {
            break;
        }
        let chunk = &buf[..read];
        if seen < 4096 {
            let peek_end = (4096 - seen).min(read);
            if chunk[..peek_end].contains(&0) {
                return Err(WorkspaceError::Tool {
                    code: "BINARY_FILE",
                    message: "Binary file read blocked for text tool.".into(),
                    category: "validation",
                    retryable: false,
                });
            }
        }
        seen += read;
        last_byte = chunk[read - 1];

        // Incremental UTF-8 validation: only an incomplete trailing sequence
        // is carried across chunks, so memory stays O(chunk).
        let mut window = Vec::with_capacity(utf8_carry.len() + chunk.len());
        window.extend_from_slice(&utf8_carry);
        window.extend_from_slice(chunk);
        utf8_carry.clear();
        let valid_len = match std::str::from_utf8(&window) {
            Ok(_) => window.len(),
            Err(error) => {
                let valid_up_to = error.valid_up_to();
                if error.error_len().is_some() {
                    return Err(unsupported_encoding());
                }
                utf8_carry.extend_from_slice(&window[valid_up_to..]);
                valid_up_to
            }
        };

        consume_stream_lines(
            &window[..valid_len],
            &mut total_lines,
            &mut selected,
            &mut line_pending,
            &mut overflow,
            start_line,
            end_line,
            max_bytes,
        );
    }

    // Trailing line without a final newline still counts (split_inclusive
    // semantics) and flushes any buffered selection bytes.
    if seen > 0 && last_byte != b'\n' {
        let line_no = total_lines + 1;
        if line_no >= start_line && end_line.map_or(true, |end| line_no <= end) {
            append_capped(&mut selected, &line_pending, max_bytes, &mut overflow);
        }
        total_lines += 1;
    }

    let end = end_line.unwrap_or(total_lines).min(total_lines);
    let truncated = overflow;
    let (content, truncated_by) = if truncated {
        let mut cut = selected.len();
        while cut > 0 && std::str::from_utf8(&selected[..cut]).is_err() {
            cut -= 1;
        }
        (
            String::from_utf8(selected[..cut].to_vec()).map_err(|_| unsupported_encoding())?,
            Some("bytes"),
        )
    } else {
        (
            String::from_utf8(selected).map_err(|_| unsupported_encoding())?,
            None,
        )
    };
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
        "path": display,
        "content": content,
        "encoding": "utf-8",
        "start_line": start_line,
        "end_line": actual_end,
        "total_lines": total_lines,
        "total_bytes": seen,
        "bytes_read": content.len(),
        "truncated": truncated,
        "truncated_by": truncated_by,
        "warnings": warnings
    })))
}

/// Feed one validated chunk through the line selector. A line ends at '\n'
/// (never split by the utf-8 carry, which cannot contain ASCII), and only
/// in-range line bytes are ever retained, capped at `max_bytes`.
#[allow(clippy::too_many_arguments)]
fn consume_stream_lines(
    window: &[u8],
    total_lines: &mut usize,
    selected: &mut Vec<u8>,
    line_pending: &mut Vec<u8>,
    overflow: &mut bool,
    start_line: usize,
    end_line: Option<usize>,
    max_bytes: usize,
) {
    let mut idx = 0;
    while idx < window.len() {
        let line_no = *total_lines + 1;
        let in_range = line_no >= start_line && end_line.map_or(true, |end| line_no <= end);
        match window[idx..].iter().position(|&b| b == b'\n') {
            Some(rel) => {
                let line_end = idx + rel + 1;
                if in_range {
                    append_capped(line_pending, &window[idx..line_end], max_bytes, overflow);
                    append_capped(selected, line_pending, max_bytes, overflow);
                    line_pending.clear();
                }
                *total_lines += 1;
                idx = line_end;
            }
            None => {
                if in_range {
                    append_capped(line_pending, &window[idx..], max_bytes, overflow);
                }
                idx = window.len();
            }
        }
    }
}

/// Append `src` to `dst` but never let `dst` exceed `max_bytes`; mark
/// `overflow` when bytes are discarded, mirroring whole-selection truncation.
fn append_capped(dst: &mut Vec<u8>, src: &[u8], max_bytes: usize, overflow: &mut bool) {
    if dst.len() >= max_bytes {
        if !src.is_empty() {
            *overflow = true;
        }
        return;
    }
    let room = max_bytes - dst.len();
    if src.len() > room {
        dst.extend_from_slice(&src[..room]);
        *overflow = true;
    } else {
        dst.extend_from_slice(src);
    }
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
    let max_file_bytes = args
        .get("max_file_bytes")
        .and_then(Value::as_u64)
        .unwrap_or(DEFAULT_SEARCH_MAX_FILE_BYTES)
        .max(1);

    let (include_globs, exclude_globs) = search_globs(args);
    let context_lines = args
        .get("context_lines")
        .and_then(Value::as_u64)
        .unwrap_or(0) as usize;
    let matcher = build_matcher(query, use_regex, case_sensitive)?;

    let mut matches = Vec::new();
    let mut warnings = Vec::new();
    let mut skipped_large = 0usize;
    let mut skipped_binary = 0usize;
    let mut truncated = false;

    let mut consider_file = |p: &Path| {
        if matches.len() >= max_results {
            truncated = true;
            return false;
        }
        if !ws.is_safe_read_path(p) {
            return true;
        }
        if ws.is_ignored_path(p, false, false) {
            return true;
        }
        let rel = relative_display(ws.root(), p);
        if !passes_glob_filters(&rel, &include_globs, &exclude_globs) {
            return true;
        }
        let meta = match p.metadata() {
            Ok(m) if m.is_file() => m,
            _ => return true,
        };
        if meta.len() > max_file_bytes {
            skipped_large += 1;
            return true;
        }
        match file_text_eligibility(p) {
            FileEligibility::Binary => {
                skipped_binary += 1;
                return true;
            }
            FileEligibility::Unreadable => return true,
            FileEligibility::Text => {}
        }
        let stop = search_file_streaming(
            p,
            &rel,
            &matcher,
            context_lines,
            max_preview,
            max_results,
            &mut matches,
        );
        if stop {
            truncated = true;
            return false;
        }
        true
    };

    if resolved.path.is_file() {
        let _ = consider_file(&resolved.path);
    } else {
        for entry in WalkDir::new(&resolved.path)
            .follow_links(false)
            .into_iter()
            .filter_map(Result::ok)
        {
            if !entry.file_type().is_file() {
                continue;
            }
            if !consider_file(entry.path()) {
                break;
            }
        }
    }

    if truncated {
        warnings.push("result limit reached; scan stopped early".to_string());
    }
    if skipped_large > 0 {
        warnings.push(format!(
            "skipped {skipped_large} file(s) larger than max_file_bytes ({max_file_bytes})"
        ));
    }
    if skipped_binary > 0 {
        warnings.push(format!(
            "skipped {skipped_binary} binary or non-utf8 file(s)"
        ));
    }

    Ok(tool_ok(json!({
        "query": query,
        "matches": matches,
        "total_matches": matches.len(),
        "truncated": truncated,
        "max_file_bytes": max_file_bytes,
        "skipped_large_files": skipped_large,
        "skipped_binary_files": skipped_binary,
        "warnings": warnings
    })))
}

enum FileEligibility {
    Text,
    Binary,
    Unreadable,
}

fn file_text_eligibility(path: &Path) -> FileEligibility {
    let mut file = match File::open(path) {
        Ok(f) => f,
        Err(_) => return FileEligibility::Unreadable,
    };
    let mut buf = [0u8; BINARY_PEEK_BYTES];
    let n = match file.read(&mut buf) {
        Ok(n) => n,
        Err(_) => return FileEligibility::Unreadable,
    };
    if buf[..n].contains(&0) {
        return FileEligibility::Binary;
    }
    FileEligibility::Text
}

/// Stream a file line-by-line. Returns true when `max_results` is reached.
fn search_file_streaming(
    path: &Path,
    rel: &str,
    matcher: &Matcher,
    context_lines: usize,
    max_preview: usize,
    max_results: usize,
    matches: &mut Vec<Value>,
) -> bool {
    let file = match File::open(path) {
        Ok(f) => f,
        Err(_) => return false,
    };
    let reader = BufReader::new(file);
    let mut recent: VecDeque<String> = VecDeque::with_capacity(context_lines.max(1));
    let mut pending: Vec<PendingMatch> = Vec::new();
    let mut line_no = 0usize;

    for line_res in reader.lines() {
        let line = match line_res {
            Ok(l) => l,
            Err(_) => {
                // Invalid UTF-8 mid-file: drop unfinished context and stop this file.
                flush_pending(&mut pending, matches, max_results);
                return matches.len() >= max_results;
            }
        };
        line_no += 1;

        // Feed "after" context for earlier hits.
        if context_lines > 0 {
            for pend in &mut pending {
                if pend.after.len() < context_lines {
                    pend.after.push(line.clone());
                }
            }
            while pending
                .first()
                .is_some_and(|front| front.after.len() >= context_lines)
            {
                let done = pending.remove(0);
                matches.push(done.into_value());
                if matches.len() >= max_results {
                    return true;
                }
            }
        }

        if matcher.is_match(&line) {
            let preview = preview_line(&line, max_preview);
            if context_lines == 0 {
                matches.push(json!({
                    "path": rel,
                    "line": line_no,
                    "column": 1,
                    "preview": preview
                }));
                if matches.len() >= max_results {
                    return true;
                }
            } else {
                pending.push(PendingMatch {
                    path: rel.to_string(),
                    line: line_no,
                    preview,
                    before: recent.iter().cloned().collect(),
                    after: Vec::new(),
                });
            }
        }

        if context_lines > 0 {
            recent.push_back(line);
            while recent.len() > context_lines {
                recent.pop_front();
            }
        }
    }

    // EOF: emit remaining pending with partial after context.
    for pend in pending {
        matches.push(pend.into_value());
        if matches.len() >= max_results {
            return true;
        }
    }
    false
}

struct PendingMatch {
    path: String,
    line: usize,
    preview: String,
    before: Vec<String>,
    after: Vec<String>,
}

impl PendingMatch {
    fn into_value(self) -> Value {
        json!({
            "path": self.path,
            "line": self.line,
            "column": 1,
            "preview": self.preview,
            "before": self.before,
            "after": self.after
        })
    }
}

fn preview_line(line: &str, max_preview: usize) -> String {
    if line.len() <= max_preview {
        return line.to_string();
    }
    let mut end = max_preview;
    while end > 0 && !line.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}...", &line[..end])
}

fn flush_pending(pending: &mut Vec<PendingMatch>, matches: &mut Vec<Value>, max_results: usize) {
    for pend in pending.drain(..) {
        if matches.len() >= max_results {
            break;
        }
        matches.push(pend.into_value());
    }
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

#[cfg(test)]
mod streaming_tests {
    use super::*;

    /// Reference (pre-fix) semantics reimplemented compactly, used as the
    /// oracle for the streaming path on inputs above the threshold.
    fn legacy_reference(content: &str, max_bytes: usize, start_line: usize, end_line: Option<usize>) -> Value {
        let lines: Vec<&str> = content.split_inclusive('\n').collect();
        let total_lines = lines.len();
        let end = end_line.unwrap_or(total_lines).min(total_lines);
        let selected: String = if end < start_line {
            String::new()
        } else {
            lines[(start_line - 1)..end].concat()
        };
        let (content, truncated, truncated_by) = {
            let bytes = selected.as_bytes();
            if bytes.len() <= max_bytes {
                (selected.clone(), false, None)
            } else {
                let mut cut = max_bytes;
                while cut > 0 && !selected.is_char_boundary(cut) {
                    cut -= 1;
                }
                (selected[..cut].to_string(), true, Some("bytes"))
            }
        };
        let actual_end = if truncated && !content.is_empty() {
            start_line + content.lines().count().saturating_sub(1)
        } else {
            end
        };
        json!({
            "content": content,
            "start_line": start_line,
            "end_line": actual_end,
            "total_lines": total_lines,
            "bytes_read": content.len(),
            "truncated": truncated,
            "truncated_by": truncated_by,
        })
    }

    fn compare(body: String, max_bytes: usize, start_line: usize, end_line: Option<usize>) {
        let dir = std::env::temp_dir().join(format!("ctm-stream-test-{}", uuid::Uuid::new_v4().simple()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("big.txt");
        std::fs::write(&path, &body).unwrap();
        let streamed = read_file_streaming(&path, "big.txt", max_bytes, start_line, end_line).unwrap();
        let expect = legacy_reference(&body, max_bytes, start_line, end_line);
        for key in [
            "content", "start_line", "end_line", "total_lines",
            "bytes_read", "truncated", "truncated_by",
        ] {
            assert_eq!(
                streamed.get(key).unwrap(),
                expect.get(key).unwrap(),
                "mismatch on {key} (max={max_bytes}, start={start_line}, end={end_line:?})"
            );
        }
        assert_eq!(streamed.get("total_bytes").unwrap().as_u64().unwrap(), body.len() as u64);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn streaming_matches_legacy_semantics() {
        let mut body = String::new();
        for i in 0..200_000 {
            body.push_str(&format!("line-{i:06} with some ASCII content plus 中文与 emoji 🌬\n"));
        }
        body.push_str("final line without newline");
        // Exercise: full read, ranges, truncation at ascii and multibyte boundaries.
        compare(body.clone(), 131_072, 1, None);
        compare(body.clone(), 131_072, 50_000, Some(150_000));
        compare(body.clone(), 7, 1, None);
        compare(body.clone(), 131_072, 200_000, None);
        compare(body.clone(), 131_072, 5, Some(3)); // end < start
        compare("no newline at all".repeat(1000), 64, 1, None); // single giant line
    }
}
