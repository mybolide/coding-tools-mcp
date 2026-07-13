use std::collections::HashSet;

use serde_json::Value;

use crate::workspace::ActionsConfig;

use super::registry::is_allowed_tool;

static FORBIDDEN_SHELL_PATTERN: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
static NETWORK_COMMAND_PATTERN: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();

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
    trimmed
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect()
}

fn default_allowed_command_set() -> HashSet<String> {
    DEFAULT_ALLOWED_COMMANDS
        .iter()
        .map(|s| s.to_string())
        .collect()
}

pub fn validate_tool_arguments(
    tool_name: &str,
    arguments: &Value,
    policy: &PolicySettings,
) -> Result<(), PolicyError> {
    match tool_name {
        "exec_command" => validate_command(arguments, policy),
        "apply_patch" => validate_patch(arguments, policy),
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
    if forbidden_shell_pattern().is_match(command) {
        return Err(PolicyError(
            "Shell chaining, redirection and expansion are not allowed".into(),
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

    let parts = shell_words::split(command)
        .map_err(|_| PolicyError("Invalid command syntax".into()))?;
    if parts.is_empty() {
        return Err(PolicyError("Empty command".into()));
    }

    let executable = parts[0].trim_start_matches("./");
    let base_name = executable
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(executable);
    let stem = base_name
        .strip_suffix(".exe")
        .or_else(|| base_name.strip_suffix(".cmd"))
        .or_else(|| base_name.strip_suffix(".bat"))
        .unwrap_or(base_name);

    if !policy.allowed_commands.contains(stem) {
        return Err(PolicyError(format!("Command is not allowlisted: {stem}")));
    }

    if matches!(stem, "python" | "python3") && parts.iter().any(|p| p == "-c") {
        return Err(PolicyError("python -c is not allowed".into()));
    }
    if stem == "node" && parts.iter().any(|p| p == "-e") {
        return Err(PolicyError("node -e is not allowed".into()));
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

fn forbidden_shell_pattern() -> &'static regex::Regex {
    FORBIDDEN_SHELL_PATTERN.get_or_init(|| {
        regex::Regex::new(r#"[;&|><`]|$\(|\$\{|[\r\n]"#).expect("valid regex")
    })
}

fn network_command_pattern() -> &'static regex::Regex {
    NETWORK_COMMAND_PATTERN.get_or_init(|| {
        regex::Regex::new(
            r"(?i)(https?://|urllib\.request|requests\.|http\.client|\bcurl\b|\bwget\b|\bssh\b|\bscp\b|\bftp\b)",
        )
        .expect("valid regex")
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
}
