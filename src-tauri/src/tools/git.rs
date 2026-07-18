use std::process::{Command, Stdio};
use std::time::Duration;

use regex::Regex;
use serde_json::{json, Value};

use crate::tools::workspace::{tool_ok, Workspace, WorkspaceError};

pub fn git_status(ws: &Workspace, args: &Value) -> Result<Value, WorkspaceError> {
    let path = args.get("path").and_then(Value::as_str).unwrap_or(".");
    let resolved = ws.resolve_existing(path)?;
    let max_entries = args
        .get("max_entries")
        .and_then(Value::as_u64)
        .unwrap_or(1000) as usize;
    let include_untracked = args
        .get("include_untracked")
        .and_then(Value::as_bool)
        .unwrap_or(true);

    let root_check = run_git(
        &resolved.path,
        &["rev-parse", "--show-toplevel"],
        Duration::from_secs(10),
    )?;
    if !root_check.success {
        return Ok(tool_ok(json!({
            "is_repo": false,
            "clean": true,
            "entries": [],
            "warnings": [root_check.stderr.trim()]
        })));
    }

    let mut status_args = vec!["status", "--porcelain=v1", "-b"];
    if !include_untracked {
        status_args.push("--untracked-files=no");
    }
    let completed = run_git(&resolved.path, &status_args, Duration::from_secs(10))?;
    if !completed.success && completed.exit_code != 0 {
        return Err(git_error(&completed.stderr));
    }

    let mut branch = String::new();
    let mut upstream = String::new();
    let mut ahead = 0i64;
    let mut behind = 0i64;
    let mut entries = Vec::new();
    let lines: Vec<_> = completed.stdout.lines().collect();
    let total_lines = lines.len();

    for line in lines {
        if let Some(rest) = line.strip_prefix("## ") {
            (branch, upstream, ahead, behind) = parse_branch_line(rest);
            continue;
        }
        if line.len() < 4 {
            continue;
        }
        let index_status = line.chars().next().unwrap_or(' ').to_string();
        let worktree_status = line.chars().nth(1).unwrap_or(' ').to_string();
        let mut path_text = line[3..].to_string();
        let original = if let Some((orig, new)) = path_text.split_once(" -> ") {
            let orig = orig.to_string();
            path_text = new.to_string();
            Some(orig)
        } else {
            None
        };
        let mut entry = json!({
            "path": path_text,
            "index_status": index_status,
            "worktree_status": worktree_status
        });
        if let Some(orig) = original {
            entry["original_path"] = json!(orig);
        }
        entries.push(entry);
        if entries.len() >= max_entries {
            break;
        }
    }

    let head = git_rev_parse(&resolved.path, "HEAD").unwrap_or_default();
    Ok(tool_ok(json!({
        "is_repo": true,
        "branch": branch,
        "head": head,
        "upstream": upstream,
        "ahead": ahead,
        "behind": behind,
        "clean": entries.is_empty(),
        "entries": entries,
        "truncated": entries.len() >= max_entries && total_lines > max_entries + 1,
        "warnings": []
    })))
}

pub fn git_diff(ws: &Workspace, args: &Value) -> Result<Value, WorkspaceError> {
    let staged = args.get("staged").and_then(Value::as_bool).unwrap_or(false);
    let unstaged = args
        .get("unstaged")
        .and_then(Value::as_bool)
        .unwrap_or(true);
    let context = args
        .get("context_lines")
        .and_then(Value::as_u64)
        .unwrap_or(3);
    let max_bytes = args
        .get("max_bytes")
        .and_then(Value::as_u64)
        .unwrap_or(262_144) as usize;

    let mut path_filters: Vec<String> = Vec::new();
    if let Some(p) = args.get("path").and_then(Value::as_str) {
        path_filters.push(p.to_string());
    }
    if let Some(paths) = args.get("paths").and_then(Value::as_array) {
        for p in paths {
            if let Some(s) = p.as_str() {
                path_filters.push(s.to_string());
            }
        }
    }
    for p in &path_filters {
        ws.reject_unsafe_text(p)?;
    }

    if !is_git_repo(ws.root()) {
        return Ok(tool_ok(json!({
            "diff": "",
            "files": [],
            "truncated": false,
            "warnings": ["not a git repository"]
        })));
    }

    let mut chunks = Vec::new();
    if unstaged {
        chunks.push(run_git_diff(ws.root(), context, &path_filters, false)?);
    }
    if staged {
        chunks.push(run_git_diff(ws.root(), context, &path_filters, true)?);
    }
    let mut combined = chunks.join("\n");
    if !combined.is_empty() && !combined.ends_with('\n') {
        combined.push('\n');
    }
    let truncated = combined.len() > max_bytes;
    let diff_text = if truncated {
        String::from_utf8_lossy(&combined.as_bytes()[..max_bytes]).into_owned()
    } else {
        combined
    };
    let files = parse_diff_files(&diff_text);
    Ok(tool_ok(json!({
        "diff": diff_text,
        "files": files,
        "truncated": truncated,
        "warnings": if truncated { vec!["diff truncated"] } else { vec![] }
    })))
}

pub fn git_log(ws: &Workspace, args: &Value) -> Result<Value, WorkspaceError> {
    let path = args.get("path").and_then(Value::as_str).unwrap_or(".");
    let resolved = ws.resolve_existing(path)?;
    let ref_name = validate_git_ref(args.get("ref").and_then(Value::as_str).unwrap_or("HEAD"))?;
    let max_count = args
        .get("max_count")
        .and_then(Value::as_u64)
        .unwrap_or(20)
        .clamp(1, 100) as usize;
    let skip = args
        .get("skip")
        .and_then(Value::as_u64)
        .unwrap_or(0)
        .min(10_000) as usize;

    if !is_git_repo(ws.root()) {
        return Ok(tool_ok(json!({
            "is_repo": false,
            "commits": [],
            "truncated": false,
            "warnings": []
        })));
    }

    let max_count_arg = format!("--max-count={}", max_count + 1);
    let skip_arg = format!("--skip={skip}");
    let pretty = "--pretty=format:%H%x1f%h%x1f%an%x1f%ae%x1f%ad%x1f%s%x1e";
    let path_filter = if resolved.display.is_empty() {
        ".".to_string()
    } else {
        resolved.display.clone()
    };
    let mut cmd_args = vec![
        "log",
        max_count_arg.as_str(),
        skip_arg.as_str(),
        "--date=iso-strict",
        pretty,
        ref_name,
    ];
    if path_filter != "." {
        cmd_args.push("--");
        cmd_args.push(path_filter.as_str());
    }

    let completed = run_git(ws.root(), &cmd_args, Duration::from_secs(10))?;
    if !completed.success {
        return Err(git_error(&completed.stderr));
    }

    let mut commits = Vec::new();
    for record in completed.stdout.split('\u{1e}') {
        let fields: Vec<String> = record
            .trim()
            .split('\u{1f}')
            .map(str::trim)
            .map(str::to_string)
            .collect();
        if fields.len() < 6 || fields[0].is_empty() {
            continue;
        }
        commits.push(json!({
            "hash": fields[0],
            "short_hash": fields[1],
            "author_name": fields[2],
            "author_email": fields[3],
            "author_date": fields[4],
            "subject": fields[5],
        }));
    }
    let truncated = commits.len() > max_count;
    Ok(tool_ok(json!({
        "is_repo": true,
        "ref": ref_name,
        "path": path_filter,
        "commits": commits.into_iter().take(max_count).collect::<Vec<_>>(),
        "truncated": truncated,
        "warnings": if truncated { vec!["commit limit reached"] } else { Vec::<&str>::new() }
    })))
}

pub fn git_show(ws: &Workspace, args: &Value) -> Result<Value, WorkspaceError> {
    if !is_git_repo(ws.root()) {
        return Ok(tool_ok(json!({
            "is_repo": false,
            "content": "",
            "files": [],
            "truncated": false,
            "warnings": []
        })));
    }

    let rev = validate_git_ref(args.get("rev").and_then(Value::as_str).unwrap_or("HEAD"))?;
    let context = args
        .get("context_lines")
        .and_then(Value::as_u64)
        .unwrap_or(3);
    let max_bytes = args
        .get("max_bytes")
        .and_then(Value::as_u64)
        .unwrap_or(262_144) as usize;
    let include_diff = args
        .get("include_diff")
        .and_then(Value::as_bool)
        .unwrap_or(true);

    let mut path_filters: Vec<String> = Vec::new();
    if let Some(p) = args.get("path").and_then(Value::as_str) {
        path_filters.push(p.to_string());
    }
    if let Some(paths) = args.get("paths").and_then(Value::as_array) {
        for p in paths {
            if let Some(s) = p.as_str() {
                path_filters.push(s.to_string());
            }
        }
    }
    for p in &path_filters {
        ws.reject_unsafe_text(p)?;
    }

    let unified = format!("--unified={context}");
    let mut cmd_args = vec!["show", "--no-ext-diff", "--format=fuller", unified.as_str()];
    if !include_diff {
        cmd_args.push("--no-patch");
    }
    cmd_args.push(rev);
    if !path_filters.is_empty() {
        cmd_args.push("--");
        for p in &path_filters {
            cmd_args.push(p.as_str());
        }
    }

    let completed = run_git(ws.root(), &cmd_args, Duration::from_secs(10))?;
    if !completed.success {
        return Err(git_error(&completed.stderr));
    }

    let truncated = completed.stdout.len() > max_bytes;
    let content = if truncated {
        String::from_utf8_lossy(&completed.stdout.as_bytes()[..max_bytes]).into_owned()
    } else {
        completed.stdout.clone()
    };
    let files = parse_diff_files(&content);
    Ok(tool_ok(json!({
        "is_repo": true,
        "rev": rev,
        "content": content,
        "files": files,
        "truncated": truncated,
        "output_bytes": content.len(),
        "warnings": if truncated { vec!["output truncated"] } else { Vec::<&str>::new() }
    })))
}

pub fn git_blame(ws: &Workspace, args: &Value) -> Result<Value, WorkspaceError> {
    let path = args
        .get("path")
        .and_then(Value::as_str)
        .ok_or_else(|| WorkspaceError::invalid_argument("path is required"))?;
    let resolved = ws.resolve_existing(path)?;
    if resolved.path.is_dir() {
        return Err(WorkspaceError::Tool {
            code: "IS_DIRECTORY",
            message: "Path is a directory.".into(),
            category: "validation",
            retryable: false,
        });
    }
    if !is_git_repo(ws.root()) {
        return Ok(tool_ok(json!({
            "is_repo": false,
            "path": resolved.display,
            "lines": [],
            "truncated": false,
            "warnings": []
        })));
    }

    let ref_arg = args.get("rev").and_then(Value::as_str);
    let git_ref = ref_arg.map(validate_git_ref).transpose()?;
    let start_line = args
        .get("start_line")
        .and_then(Value::as_u64)
        .unwrap_or(1)
        .max(1) as usize;
    let end_line_arg = args
        .get("end_line")
        .and_then(Value::as_u64)
        .map(|v| v as usize);
    let max_lines = args
        .get("max_lines")
        .and_then(Value::as_u64)
        .unwrap_or(200)
        .clamp(1, 1000) as usize;

    let final_line = match end_line_arg {
        None => start_line + max_lines - 1,
        Some(end) if end < start_line => {
            return Err(WorkspaceError::invalid_argument(
                "end_line must be >= start_line.",
            ));
        }
        Some(end) => end,
    };
    let requested_lines = final_line - start_line + 1;
    let mut truncated = requested_lines > max_lines;
    let final_line = final_line.min(start_line + max_lines - 1);

    let line_range = format!("{start_line},{final_line}");
    let mut cmd_args = vec!["blame", "--line-porcelain", "-L", line_range.as_str()];
    if let Some(r) = git_ref {
        cmd_args.push(r);
    }
    cmd_args.push("--");
    cmd_args.push(resolved.display.as_str());

    let completed = run_git(ws.root(), &cmd_args, Duration::from_secs(10))?;
    if !completed.success {
        return Err(git_error(&completed.stderr));
    }

    let mut lines = parse_git_blame_porcelain(&completed.stdout);
    if lines.len() > max_lines {
        lines.truncate(max_lines);
        truncated = true;
    }

    Ok(tool_ok(json!({
        "is_repo": true,
        "path": resolved.display,
        "rev": ref_arg,
        "start_line": start_line,
        "end_line": final_line,
        "lines": lines,
        "truncated": truncated,
        "warnings": if truncated { vec!["line limit reached"] } else { Vec::<&str>::new() }
    })))
}

fn validate_git_ref(ref_name: &str) -> Result<&str, WorkspaceError> {
    if ref_name.is_empty()
        || ref_name.starts_with('-')
        || ref_name.contains('\0')
        || ref_name.contains('\n')
        || ref_name.contains('\r')
    {
        return Err(WorkspaceError::invalid_argument("Invalid git revision."));
    }
    Ok(ref_name)
}

fn parse_git_blame_porcelain(output: &str) -> Vec<Value> {
    let commit_re = Regex::new(r"^[0-9a-fA-F^]{40}").expect("valid regex");
    let mut rows = Vec::new();
    let mut current: serde_json::Map<String, Value> = serde_json::Map::new();

    for raw in output.lines() {
        let parts: Vec<&str> = raw.split_whitespace().collect();
        if parts.len() >= 3 && commit_re.is_match(parts[0]) {
            current = serde_json::Map::new();
            current.insert("commit".into(), json!(parts[0].trim_start_matches('^')));
            if parts[1].chars().all(|c| c.is_ascii_digit()) {
                current.insert("original_line".into(), json!(parts[1].parse::<i64>().ok()));
            }
            if parts[2].chars().all(|c| c.is_ascii_digit()) {
                current.insert("line".into(), json!(parts[2].parse::<i64>().ok()));
            }
            continue;
        }
        if let Some(author) = raw.strip_prefix("author ") {
            current.insert("author".into(), json!(author));
            continue;
        }
        if let Some(mail) = raw.strip_prefix("author-mail ") {
            current.insert(
                "author_mail".into(),
                json!(mail.trim_matches(|c| c == '<' || c == '>')),
            );
            continue;
        }
        if let Some(time) = raw.strip_prefix("author-time ") {
            let value = if time.chars().all(|c| c.is_ascii_digit()) {
                json!(time.parse::<i64>().ok())
            } else {
                json!(time)
            };
            current.insert("author_time".into(), value);
            continue;
        }
        if let Some(summary) = raw.strip_prefix("summary ") {
            current.insert("summary".into(), json!(summary));
            continue;
        }
        if let Some(content) = raw.strip_prefix('\t') {
            let mut row = current.clone();
            row.insert("content".into(), json!(content));
            rows.push(Value::Object(row));
        }
    }
    rows
}

struct GitOutput {
    success: bool,
    exit_code: i32,
    stdout: String,
    stderr: String,
}

fn run_git(
    cwd: &std::path::Path,
    args: &[&str],
    limit: Duration,
) -> Result<GitOutput, WorkspaceError> {
    let mut cmd = Command::new("git");
    cmd.arg("-C")
        .arg(cwd)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let output = cmd
        .output()
        .map_err(|e| git_error(&format!("git not available: {e}")))?;
    let _ = limit;
    Ok(GitOutput {
        success: output.status.success(),
        exit_code: output.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    })
}

fn run_git_diff(
    root: &std::path::Path,
    context: u64,
    path_filters: &[String],
    cached: bool,
) -> Result<String, WorkspaceError> {
    let unified = format!("--unified={context}");
    let mut args = vec!["diff", unified.as_str()];
    if cached {
        args.push("--cached");
    }
    if !path_filters.is_empty() {
        args.push("--");
        for p in path_filters {
            args.push(p.as_str());
        }
    }
    let completed = run_git(root, &args, Duration::from_secs(10))?;
    if completed.exit_code != 0 && completed.exit_code != 1 {
        return Err(git_error(&completed.stderr));
    }
    Ok(completed.stdout)
}

fn is_git_repo(root: &std::path::Path) -> bool {
    run_git(root, &["rev-parse", "--git-dir"], Duration::from_secs(5))
        .map(|o| o.success)
        .unwrap_or(false)
}

fn git_rev_parse(cwd: &std::path::Path, rev: &str) -> Option<String> {
    run_git(cwd, &["rev-parse", rev], Duration::from_secs(5))
        .ok()
        .filter(|o| o.success)
        .map(|o| o.stdout.trim().to_string())
}

fn parse_branch_line(line: &str) -> (String, String, i64, i64) {
    let (branch_part, tracking) = line
        .split_once("...")
        .map(|(b, t)| (b.to_string(), t.to_string()))
        .unwrap_or((line.to_string(), String::new()));
    let branch = branch_part
        .split_once(' ')
        .map(|(b, _)| b.to_string())
        .unwrap_or(branch_part);
    let mut ahead = 0i64;
    let mut behind = 0i64;
    let mut upstream = tracking.clone();
    if let Some(idx) = tracking.find(' ') {
        upstream = tracking[..idx].to_string();
        let meta = &tracking[idx + 1..];
        for token in meta.split(',') {
            let token = token.trim();
            if let Some(n) = token.strip_prefix("ahead ") {
                ahead = n.trim().parse().unwrap_or(0);
            } else if let Some(n) = token.strip_prefix("behind ") {
                behind = n.trim().parse().unwrap_or(0);
            }
        }
    }
    (branch, upstream, ahead, behind)
}

fn parse_diff_files(diff: &str) -> Vec<Value> {
    let mut files = Vec::new();
    for line in diff.lines() {
        if let Some(path) = line.strip_prefix("+++ b/") {
            files.push(json!({
                "path": path,
                "status": "modified",
                "binary": false
            }));
        } else if line.starts_with("--- /dev/null") {
            continue;
        } else if let Some(path) = line.strip_prefix("--- a/") {
            if !files.iter().any(|f| f["path"] == path) {
                files.push(json!({
                    "path": path,
                    "status": "modified",
                    "binary": false
                }));
            }
        }
    }
    files
}

fn git_error(message: &str) -> WorkspaceError {
    WorkspaceError::Tool {
        code: "GIT_ERROR",
        message: message.to_string(),
        category: "runtime",
        retryable: false,
    }
}
