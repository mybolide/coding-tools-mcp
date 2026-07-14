use std::collections::HashSet;
use std::path::{Component, Path};

use serde_json::Value;

use crate::workspace::ActionsConfig;

use super::registry::is_allowed_tool;

static NETWORK_COMMAND_PATTERN: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
static DANGEROUS_COMMAND_PATTERN: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
static INTERPRETER_MUTATION_PATTERN: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();

const BASIC_READ_ONLY_COMMANDS: &[&str] = &[
    "pwd", "ls", "dir", "cat", "head", "tail", "grep", "find", "which", "echo",
];

const DEFAULT_ALLOWED_COMMANDS: &[&str] = &[
    "pytest", "python", "python3", "npm", "npx", "node", "pnpm", "yarn", "make", "mvn", "mvnw",
    "gradle", "gradlew", "cargo", "go", "ruff", "mypy", "eslint", "tsc", "msbuild", "dotnet",
    "deno", "bun", "ruby", "java", "javac", "cmake", "clang", "gcc", "g++", "git",
];

#[derive(Debug, Clone)]
pub struct PolicySettings {
    pub allowed_commands: HashSet<String>,
    pub max_patch_bytes: usize,
    pub permission_mode: String,
}

impl Default for PolicySettings {
    fn default() -> Self {
        Self {
            allowed_commands: default_allowed_command_set(),
            max_patch_bytes: 200_000,
            permission_mode: "trusted".into(),
        }
    }
}

impl PolicySettings {
    pub fn from_runtime(runtime: &crate::workspace::RuntimeConfig) -> Self {
        Self {
            allowed_commands: default_allowed_command_set(),
            max_patch_bytes: 200_000,
            permission_mode: runtime.permission_mode.clone(),
        }
    }

    pub fn from_actions_config(actions: &ActionsConfig) -> Self {
        Self {
            allowed_commands: parse_allowed_commands(&actions.allowed_commands),
            max_patch_bytes: actions.max_patch_bytes as usize,
            permission_mode: actions.permission_mode.clone(),
        }
    }

    pub fn network_allowed(&self) -> bool {
        self.permission_mode == "trusted" || self.permission_mode == "dangerous"
    }

    pub fn skip_permission_gates(&self) -> bool {
        self.permission_mode == "dangerous"
    }
}

#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub struct PolicyError(pub String);

pub fn parse_allowed_commands(configured: &str) -> HashSet<String> {
    let trimmed = configured.trim();
    if trimmed.is_empty() {
        return default_allowed_command_set();
    }
    let mut commands: HashSet<String> = trimmed
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect();
    // 基础诊断命令是工作区可用性的最低保障，不应因 Actions 配置遗漏而失效。
    commands.extend(BASIC_READ_ONLY_COMMANDS.iter().map(|s| s.to_string()));
    commands
}

fn default_allowed_command_set() -> HashSet<String> {
    DEFAULT_ALLOWED_COMMANDS
        .iter()
        .map(|s| s.to_string())
        .chain(BASIC_READ_ONLY_COMMANDS.iter().map(|s| s.to_string()))
        .collect()
}

pub fn validate_tool_arguments(
    tool_name: &str,
    arguments: &Value,
    policy: &PolicySettings,
) -> Result<(), PolicyError> {
    match tool_name {
        "exec_command" => validate_command(arguments, policy),
        "apply_patch" | "patch_check" => validate_patch(arguments, policy),
        _ => Ok(()),
    }
}

/// Actions OpenAPI 暴露层校验：仅限制「能否调用」，不参与执行逻辑。
pub fn validate_actions_exposure(tool_name: &str) -> Result<(), PolicyError> {
    if is_allowed_tool(tool_name) {
        Ok(())
    } else {
        Err(PolicyError(format!("Tool is not exposed: {tool_name}")))
    }
}

pub fn validate_command(arguments: &Value, policy: &PolicySettings) -> Result<(), PolicyError> {
    let command = arguments
        .get("cmd")
        .and_then(Value::as_str)
        .ok_or_else(|| PolicyError("exec_command requires a non-empty cmd".into()))?;
    if command.trim().is_empty() {
        return Err(PolicyError("exec_command requires a non-empty cmd".into()));
    }
    if command.len() > 4_000 {
        return Err(PolicyError("Command is too long".into()));
    }
    let filesystem_scope = arguments
        .get("filesystem_scope")
        .and_then(Value::as_str)
        .unwrap_or("workspace");
    if filesystem_scope != "workspace" {
        return Err(PolicyError(
            "EXTERNAL_EXECUTION_NOT_ALLOWED: exec_command 只允许在 Workspace 内执行".into(),
        ));
    }
    for key in ["workdir", "cwd"] {
        if let Some(workdir) = arguments.get(key).and_then(Value::as_str) {
            let path = Path::new(workdir);
            if path.is_absolute() || path.components().any(|part| part == Component::ParentDir) {
                return Err(PolicyError(
                    "workdir must stay inside the configured workspace".into(),
                ));
            }
        }
    }
    if has_forbidden_shell_syntax(command) {
        return Err(PolicyError(
            "Shell chaining, redirection and expansion are not allowed".into(),
        ));
    }
    if (dangerous_command_pattern().is_match(command)
        || interpreter_mutation_pattern().is_match(command))
        && command_targets_protected_repository_asset(command)
    {
        return Err(PolicyError(
            "PROTECTED_REPOSITORY_ASSET: 禁止删除或递归清空 .git/.github".into(),
        ));
    }
    if interpreter_mutation_pattern().is_match(command)
        && command_contains_external_path(command)
    {
        return Err(PolicyError(
            "WORKSPACE_PATH_PROTECTED: workspace scope 禁止通过子进程写入 Workspace 外部路径".into(),
        ));
    }
    if dangerous_command_pattern().is_match(command)
        && !arguments
            .get("confirm")
            .and_then(Value::as_bool)
            .unwrap_or(false)
    {
        return Err(PolicyError(
            "DANGEROUS_OPERATION_REQUIRES_CONFIRMATION: dangerous command requires confirm=true"
                .into(),
        ));
    }
    if !policy.skip_permission_gates()
        && network_command_pattern().is_match(command)
        && !policy.network_allowed()
    {
        return Err(PolicyError(
            "Network-looking commands are blocked in safe permission mode".into(),
        ));
    }

    let parts =
        shell_words::split(command).map_err(|_| PolicyError("Invalid command syntax".into()))?;
    if parts.is_empty() {
        return Err(PolicyError("Empty command".into()));
    }

    let executable = parts[0].trim_start_matches("./");
    let base_name = executable.rsplit(['/', '\\']).next().unwrap_or(executable);
    let stem = base_name
        .strip_suffix(".exe")
        .or_else(|| base_name.strip_suffix(".cmd"))
        .or_else(|| base_name.strip_suffix(".bat"))
        .unwrap_or(base_name);

    if !policy.allowed_commands.contains(stem) {
        return Err(PolicyError(format!("Command is not allowlisted: {stem}")));
    }

    if arguments.get("env").is_some() {
        return Err(PolicyError(
            "Environment variables cannot be supplied by GPT".into(),
        ));
    }

    if let Some(timeout_ms) = arguments.get("timeout_ms").and_then(Value::as_u64) {
        if timeout_ms > 600_000 {
            return Err(PolicyError("Command timeout exceeds 10 minutes".into()));
        }
    }

    Ok(())
}

pub fn validate_patch(arguments: &Value, policy: &PolicySettings) -> Result<(), PolicyError> {
    let patch = arguments
        .get("patch")
        .and_then(Value::as_str)
        .ok_or_else(|| PolicyError("apply_patch requires a patch".into()))?;
    if patch.trim().is_empty() {
        return Err(PolicyError("apply_patch requires a patch".into()));
    }

    if patch.len() > policy.max_patch_bytes {
        return Err(PolicyError("Patch is too large".into()));
    }

    Ok(())
}

fn has_forbidden_shell_syntax(command: &str) -> bool {
    if command.contains(['\r', '\n']) {
        return true;
    }

    let chars: Vec<char> = command.chars().collect();
    let mut quote = None;
    let mut escaped = false;
    let mut index = 0;
    while index < chars.len() {
        let ch = chars[index];
        if escaped {
            escaped = false;
            index += 1;
            continue;
        }

        match quote {
            Some('\'') => {
                if ch == '\'' {
                    quote = None;
                }
            }
            Some('"') => {
                if ch == '\\' {
                    escaped = true;
                } else if ch == '"' {
                    quote = None;
                }
            }
            Some(_) => {}
            None => {
                if ch == '\\' {
                    escaped = true;
                } else if ch == '\'' || ch == '"' {
                    quote = Some(ch);
                } else if matches!(ch, ';' | '&' | '|' | '>' | '<' | '`')
                    || (ch == '$'
                        && chars
                            .get(index + 1)
                            .is_some_and(|next| *next == '(' || *next == '{'))
                {
                    return true;
                }
            }
        }
        index += 1;
    }
    false
}

fn network_command_pattern() -> &'static regex::Regex {
    NETWORK_COMMAND_PATTERN.get_or_init(|| {
        regex::Regex::new(
            r"(?i)(https?://|urllib\.request|requests\.|http\.client|\bcurl\b|\bwget\b|\bssh\b|\bscp\b|\bftp\b)",
        )
        .expect("valid regex")
    })
}

fn dangerous_command_pattern() -> &'static regex::Regex {
    DANGEROUS_COMMAND_PATTERN.get_or_init(|| {
        regex::Regex::new(
            r"(?i)(git\s+reset\s+--hard|git\s+clean\s+-[^\r\n]*f|git\s+checkout\s+--\s+\.|(^|\s)rm\s+(-[^\r\n]*r[^\r\n]*f|--recursive)|remove-item\s+[^\r\n]*-recurse|(^|\s)(rmdir|del)\s+/s\b)",
        )
        .expect("valid regex")
    })
}

fn interpreter_mutation_pattern() -> &'static regex::Regex {
    INTERPRETER_MUTATION_PATTERN.get_or_init(|| {
        regex::Regex::new(
            r#"(?i)(shutil\.(rmtree|move)|os\.(remove|unlink|rmdir)|pathlib\.[^\s;]+\.(unlink|rename)|write_text|write_bytes|fs\.(writefile|writefilesync|unlink|rm)|set-content|out-file|new-item|files?\.(write|delete)|open\([^)]*['\"]w)"#,
        )
        .expect("valid regex")
    })
}

fn command_contains_external_path(command: &str) -> bool {
    let normalized = command.replace('\\', "/");
    normalized.contains("../")
        || normalized.contains("..\\")
        || regex::Regex::new(r#"(?i)(^|["'\s])/[^"]"#)
            .expect("valid regex")
            .is_match(&normalized)
        || regex::Regex::new(r"(?i)\b[A-Z]:/")
            .expect("valid regex")
            .is_match(&normalized)
}

fn command_targets_protected_repository_asset(command: &str) -> bool {
    let normalized_command = command.to_ascii_lowercase().replace('\\', "/");
    let references_protected_asset = normalized_command.contains(".git")
        || normalized_command.contains(".github");
    if !references_protected_asset {
        return false;
    }

    let mutating_operation = [
        "rm ",
        "remove-item",
        "rmdir",
        "del ",
        "unlink",
        "rmtree",
        "write_text",
        "writefile",
        "rename",
        "move",
        "checkout",
        "clean ",
    ]
    .iter()
    .any(|needle| normalized_command.contains(needle));
    if mutating_operation {
        return true;
    }

    command.split_whitespace().any(|part| {
        let token = part
            .trim_matches(|ch: char| matches!(ch, '\'' | '"' | '`' | ',' | ';'))
            .replace('\\', "/");
        let token = token.strip_prefix("./").unwrap_or(&token);
        token == ".git"
            || token.starts_with(".git/")
            || token == ".github"
            || token.starts_with(".github/")
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn workspace_allowed_commands_override_defaults() {
        let actions = ActionsConfig {
            allowed_commands: "cargo,go".into(),
            ..ActionsConfig::default()
        };
        let policy = PolicySettings::from_actions_config(&actions);
        assert!(policy.allowed_commands.contains("cargo"));
        assert!(!policy.allowed_commands.contains("pytest"));
    }

    #[test]
    fn patch_size_uses_workspace_limit() {
        let actions = ActionsConfig {
            max_patch_bytes: 10,
            ..ActionsConfig::default()
        };
        let policy = PolicySettings::from_actions_config(&actions);
        let err = validate_patch(&json!({ "patch": "01234567890" }), &policy).unwrap_err();
        assert!(err.0.contains("too large"));
    }

    #[test]
    fn basic_diagnostic_commands_are_allowed() {
        let policy = PolicySettings::default();
        for command in BASIC_READ_ONLY_COMMANDS {
            validate_command(&json!({"cmd": command}), &policy)
                .unwrap_or_else(|err| panic!("{command} should be allowed: {err}"));
        }
    }

    #[test]
    fn configured_commands_keep_basic_diagnostics() {
        let actions = ActionsConfig {
            allowed_commands: "cargo,go".into(),
            ..ActionsConfig::default()
        };
        let policy = PolicySettings::from_actions_config(&actions);
        assert!(validate_command(&json!({"cmd": "pwd"}), &policy).is_ok());
        assert!(validate_command(&json!({"cmd": "pytest"}), &policy).is_err());
    }

    #[test]
    fn quoted_python_code_is_not_treated_as_shell_chaining() {
        let policy = PolicySettings::default();
        assert!(validate_command(
            &json!({"cmd": "python -c \"import os; print(os.getcwd())\""}),
            &policy
        )
        .is_ok());
        assert!(validate_command(
            &json!({"cmd": "python -c \"print(1)\" && echo nope"}),
            &policy
        )
        .is_err());
        assert!(validate_command(
            &json!({"cmd": "echo hello > output.txt"}),
            &policy
        )
        .is_err());
    }
}
