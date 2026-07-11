use serde_json::{json, Value};

pub const P0_TOOLS: &[(&str, &str, &str, bool, bool, bool)] = &[
    ("server_info", "Server info", "Return server, workspace, auth, profile, and exposed-tool metadata.", true, false, false),
    ("check_exec_environment", "Check exec environment", "Return lightweight exec_command sandbox and environment status known to the server.", true, false, false),
    ("get_default_cwd", "Get default cwd", "Return the current default cwd inside the workspace.", true, false, false),
    ("set_default_cwd", "Set default cwd", "Set the default cwd for relative tool paths inside the workspace.", true, false, false),
    ("read_file", "Read file", "Read a UTF-8 text file slice inside the configured workspace.", true, false, false),
    ("list_dir", "List directory", "List directory entries inside the configured workspace.", true, false, false),
    ("list_files", "List files", "List workspace files using glob filters.", true, false, false),
    ("search_text", "Search text", "Search UTF-8 workspace files for text or regex matches.", true, false, false),
    ("apply_patch", "Apply patch", "Apply a patch envelope transactionally inside the workspace.", false, true, false),
    ("exec_command", "Execute command", "Run a bounded command in the workspace under runtime policy.", false, true, true),
    ("write_stdin", "Write stdin", "Write characters to a server-managed running command session.", false, false, false),
    ("kill_session", "Kill session", "Terminate a server-managed running command session.", false, true, false),
    ("read_output", "Read output", "Read retained stdout or stderr by output_ref with per-stream byte offset pagination.", true, false, false),
    ("git_status", "Git status", "Return git working tree status for the workspace.", true, false, false),
    ("git_diff", "Git diff", "Return unified git diff for workspace changes.", true, false, false),
    ("git_log", "Git log", "Return recent git commits with bounded structured metadata.", true, false, false),
    ("git_show", "Git show", "Return bounded git show output for a revision.", true, false, false),
    ("git_blame", "Git blame", "Return bounded git blame metadata for a workspace file.", true, false, false),
    ("request_permissions", "Request permissions", "Request a scoped permission grant for dangerous runtime operations.", true, false, false),
    ("view_image", "View image", "Return a workspace image as MCP image content.", true, false, false),
];

pub const ALLOWED_TOOLS: &[&str] = &[
    "server_info",
    "check_exec_environment",
    "read_file",
    "list_dir",
    "list_files",
    "search_text",
    "apply_patch",
    "exec_command",
    "read_output",
    "git_status",
    "git_diff",
    "git_log",
    "git_show",
    "git_blame",
];

pub const MUTATING_TOOLS: &[&str] = &["apply_patch", "exec_command"];

pub const READ_ONLY_TOOLS: &[&str] = &[
    "server_info",
    "check_exec_environment",
    "get_default_cwd",
    "set_default_cwd",
    "read_file",
    "list_dir",
    "list_files",
    "search_text",
    "read_output",
    "git_status",
    "git_diff",
    "git_log",
    "git_show",
    "git_blame",
    "request_permissions",
    "view_image",
];

pub fn is_allowed_tool(name: &str) -> bool {
    ALLOWED_TOOLS.contains(&name)
}

pub fn exposed_tool_names(tool_profile: &str) -> Vec<&'static str> {
    if tool_profile == "read-only" {
        READ_ONLY_TOOLS.to_vec()
    } else {
        P0_TOOLS.iter().map(|(name, ..)| *name).collect()
    }
}

pub fn list_tools() -> Vec<Value> {
    list_tools_for_profile("full")
}

pub fn list_tools_for_profile(tool_profile: &str) -> Vec<Value> {
    let compat = tool_profile == "compat-readonly-all";
    exposed_tool_names(tool_profile)
        .into_iter()
        .filter_map(|name| {
            P0_TOOLS.iter().find(|(n, ..)| *n == name).map(|entry| {
                let (name, title, description, read_only, destructive, open_world) = *entry;
                let (read_only, destructive, open_world) = if compat {
                    (true, false, false)
                } else {
                    (read_only, destructive, open_world)
                };
                json!({
                    "name": name,
                    "title": title,
                    "description": description,
                    "inputSchema": input_schema(name),
                    "annotations": {
                        "title": title,
                        "readOnlyHint": read_only,
                        "destructiveHint": destructive,
                        "idempotentHint": read_only,
                        "openWorldHint": open_world
                    }
                })
            })
        })
        .collect()
}

pub fn input_schema(name: &str) -> Value {
    match name {
        "read_file" => json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "minLength": 1 },
                "start_line": { "type": "integer", "minimum": 1, "default": 1 },
                "end_line": { "type": "integer", "minimum": 1 },
                "max_bytes": { "type": "integer", "minimum": 1, "maximum": 1048576, "default": 131072 }
            },
            "required": ["path"],
            "additionalProperties": false
        }),
        "list_dir" => json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "default": "." },
                "recursive": { "type": "boolean", "default": false },
                "max_depth": { "type": "integer", "minimum": 1, "maximum": 20, "default": 1 },
                "max_entries": { "type": "integer", "minimum": 1, "maximum": 10000, "default": 1000 },
                "include_hidden": { "type": "boolean", "default": false },
                "include_ignored": { "type": "boolean", "default": false }
            },
            "additionalProperties": false
        }),
        "list_files" => json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "default": "." },
                "patterns": { "type": "array", "items": { "type": "string" } },
                "glob": { "type": "string", "description": "Alias for a single patterns entry" },
                "exclude_patterns": { "type": "array", "items": { "type": "string" } },
                "include_hidden": { "type": "boolean", "default": false },
                "include_ignored": { "type": "boolean", "default": false },
                "max_results": { "type": "integer", "minimum": 1, "maximum": 50000, "default": 5000 }
            },
            "additionalProperties": false
        }),
        "search_text" => json!({
            "type": "object",
            "properties": {
                "query": { "type": "string", "minLength": 1 },
                "path": { "type": "string", "default": "." },
                "glob": { "type": "string", "description": "Alias appended to include_globs" },
                "include_globs": { "type": "array", "items": { "type": "string" } },
                "exclude_globs": { "type": "array", "items": { "type": "string" } },
                "regex": { "type": "boolean", "default": false },
                "case_sensitive": { "type": "boolean", "default": false },
                "context_lines": { "type": "integer", "minimum": 0, "maximum": 20, "default": 0 },
                "max_preview_bytes": { "type": "integer", "minimum": 64, "maximum": 4096, "default": 512 },
                "max_results": { "type": "integer", "minimum": 1, "maximum": 10000, "default": 1000 }
            },
            "required": ["query"],
            "additionalProperties": false
        }),
        "apply_patch" => json!({
            "type": "object",
            "properties": {
                "patch": { "type": "string", "minLength": 1 },
                "dry_run": { "type": "boolean", "default": false }
            },
            "required": ["patch"],
            "additionalProperties": false
        }),
        "exec_command" => json!({
            "type": "object",
            "properties": {
                "cmd": { "type": "string", "minLength": 1 },
                "workdir": { "type": "string", "default": "." },
                "timeout_ms": { "type": "integer", "minimum": 1, "maximum": 600000, "default": 30000 },
                "max_output_bytes": { "type": "integer", "minimum": 1024, "maximum": 1048576, "default": 65536 },
                "yield_time_ms": { "type": "integer", "minimum": 0, "maximum": 30000, "default": 1000 },
                "tty": { "type": "boolean", "default": false },
                "stdin": { "type": "string", "default": "" }
            },
            "required": ["cmd"],
            "additionalProperties": false
        }),
        "write_stdin" => json!({
            "type": "object",
            "properties": {
                "session_id": { "type": "string", "minLength": 1 },
                "chars": { "type": "string", "default": "" },
                "yield_time_ms": { "type": "integer", "minimum": 0, "maximum": 30000, "default": 1000 },
                "max_output_bytes": { "type": "integer", "minimum": 1, "maximum": 1048576, "default": 65536 }
            },
            "required": ["session_id"],
            "additionalProperties": false
        }),
        "kill_session" => json!({
            "type": "object",
            "properties": {
                "session_id": { "type": "string", "minLength": 1 },
                "signal": { "type": "string", "enum": ["TERM", "KILL", "INT"], "default": "TERM" },
                "wait_ms": { "type": "integer", "minimum": 0, "maximum": 30000, "default": 5000 },
                "max_output_bytes": { "type": "integer", "minimum": 1, "maximum": 1048576, "default": 65536 }
            },
            "required": ["session_id"],
            "additionalProperties": false
        }),
        "read_output" => json!({
            "type": "object",
            "properties": {
                "output_ref": { "type": "string", "minLength": 1 },
                "stream": { "type": "string", "enum": ["stdout", "stderr"] },
                "offset": { "type": "integer", "minimum": 0, "default": 0 },
                "limit": { "type": "integer", "minimum": 1, "maximum": 1048576, "default": 4096 }
            },
            "required": ["output_ref"],
            "additionalProperties": false
        }),
        "git_status" => json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "default": "." },
                "include_untracked": { "type": "boolean", "default": true },
                "max_entries": { "type": "integer", "minimum": 1, "maximum": 10000, "default": 1000 }
            },
            "additionalProperties": false
        }),
        "git_diff" => json!({
            "type": "object",
            "properties": {
                "paths": { "type": "array", "items": { "type": "string" }, "default": [] },
                "staged": { "type": "boolean", "default": false },
                "unstaged": { "type": "boolean", "default": true },
                "context_lines": { "type": "integer", "minimum": 0, "maximum": 20, "default": 3 },
                "max_bytes": { "type": "integer", "minimum": 1024, "maximum": 1048576, "default": 262144 }
            },
            "additionalProperties": false
        }),
        "git_log" => json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "default": "." },
                "ref": { "type": "string", "default": "HEAD" },
                "max_count": { "type": "integer", "minimum": 1, "maximum": 100, "default": 20 },
                "skip": { "type": "integer", "minimum": 0, "maximum": 10000, "default": 0 }
            },
            "additionalProperties": false
        }),
        "git_show" => json!({
            "type": "object",
            "properties": {
                "rev": { "type": "string", "default": "HEAD" },
                "path": { "type": "string" },
                "paths": { "type": "array", "items": { "type": "string" } },
                "include_diff": { "type": "boolean", "default": true },
                "context_lines": { "type": "integer", "minimum": 0, "maximum": 20, "default": 3 },
                "max_bytes": { "type": "integer", "minimum": 1, "maximum": 1048576, "default": 262144 }
            },
            "additionalProperties": false
        }),
        "git_blame" => json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "minLength": 1 },
                "rev": { "type": "string" },
                "start_line": { "type": "integer", "minimum": 1, "default": 1 },
                "end_line": { "type": "integer", "minimum": 1 },
                "max_lines": { "type": "integer", "minimum": 1, "maximum": 1000, "default": 200 }
            },
            "required": ["path"],
            "additionalProperties": false
        }),
        "set_default_cwd" => json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "default": "." }
            },
            "additionalProperties": false
        }),
        "view_image" => json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "minLength": 1 },
                "max_bytes": { "type": "integer", "minimum": 1024, "maximum": 10485760, "default": 5242880 },
                "max_width": { "type": "integer", "minimum": 1, "maximum": 10000, "default": 2000 },
                "max_height": { "type": "integer", "minimum": 1, "maximum": 10000, "default": 2000 },
                "auto_resize": { "type": "boolean", "default": true },
                "output": { "type": "string", "enum": ["mcp_image", "data_url"], "default": "mcp_image" }
            },
            "required": ["path"],
            "additionalProperties": false
        }),
        _ => json!({
            "type": "object",
            "properties": {},
            "additionalProperties": false
        }),
    }
}
