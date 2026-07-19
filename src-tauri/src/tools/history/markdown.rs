use std::sync::OnceLock;

use regex::Regex;
use serde_json::Value;
use sha2::{Digest, Sha256};

use super::model::CheckpointRecord;

const CHECKPOINT_HEADING: &str = "## 本轮检查点";
const INHERITED_SUMMARY_HEADING: &str = "继承的历史摘要";

pub fn metadata(content: &str, label: &str) -> Option<String> {
    let prefix = format!("**{label}:**");
    content.lines().find_map(|line| {
        line.trim()
            .strip_prefix(&prefix)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
    })
}

pub fn document_title(content: &str, number: u64) -> String {
    content
        .lines()
        .find_map(|line| line.trim().strip_prefix("# "))
        .and_then(|line| line.split_once('：').map(|(_, title)| title.trim()))
        .filter(|title| !title.is_empty())
        .unwrap_or("开发会话")
        .to_string()
        .replace(&format!("会话 {number}"), "")
        .trim_matches(['：', ':', ' '])
        .to_string()
}

pub fn render_document(
    number: u64,
    title: &str,
    session_key: &str,
    created_at: &str,
    updated_at: &str,
    status: &str,
    records: &[CheckpointRecord],
) -> String {
    let title = if title.trim().is_empty() {
        "开发会话"
    } else {
        title.trim()
    };
    let mut output = format!(
        "# 会话 {number}：{title}\n\n\
**Session key:** {session_key}\n\
**Created:** {created_at}\n\
**Updated:** {updated_at}\n\
**Status:** {status}\n\n"
    );
    push_section(
        &mut output,
        "用户核心目标",
        records
            .iter()
            .map(|record| record.user_intent.as_str())
            .filter(|value| !value.is_empty()),
    );
    push_section(
        &mut output,
        "已确认事实",
        records
            .iter()
            .flat_map(|record| record.findings.iter().map(String::as_str)),
    );
    push_section(
        &mut output,
        "已完成修改",
        records
            .iter()
            .flat_map(|record| record.files_changed.iter().map(String::as_str)),
    );
    push_section(
        &mut output,
        "关键设计决定",
        records
            .iter()
            .flat_map(|record| record.decisions.iter().map(String::as_str)),
    );
    push_section(
        &mut output,
        "测试结果",
        records
            .iter()
            .flat_map(|record| record.tests.iter().map(String::as_str)),
    );
    push_section(
        &mut output,
        "当前运行状态",
        records
            .iter()
            .flat_map(|record| record.runtime_state.iter().map(String::as_str)),
    );
    push_section(
        &mut output,
        "剩余问题",
        records
            .iter()
            .flat_map(|record| record.remaining_issues.iter().map(String::as_str)),
    );
    push_section(
        &mut output,
        "下一步",
        records
            .iter()
            .flat_map(|record| record.next_actions.iter().map(String::as_str)),
    );
    output.push_str(CHECKPOINT_HEADING);
    output.push_str("\n\n");
    for record in records {
        output.push_str("### ");
        output.push_str(&record.turn_id);
        output.push_str("\n\n```json\n");
        output.push_str(
            &serde_json::to_string_pretty(record).expect("checkpoint record is serializable"),
        );
        output.push_str("\n```\n\n");
    }
    output
}

pub fn attach_inherited_summary(mut content: String, summary: &str) -> String {
    let summary = summary.trim();
    if summary.is_empty() {
        return content;
    }
    let Some(status_start) = content.find("**Status:**") else {
        return content;
    };
    let Some(relative_end) = content[status_start..].find("\n\n") else {
        return content;
    };
    let insert_at = status_start + relative_end + 2;
    content.insert_str(
        insert_at,
        &format!("## {INHERITED_SUMMARY_HEADING}\n\n{summary}\n\n"),
    );
    content
}

pub fn inherited_summary(content: &str) -> Option<String> {
    section_body(content, INHERITED_SUMMARY_HEADING)
        .map(str::trim)
        .filter(|summary| !summary.is_empty())
        .map(str::to_string)
}

fn push_section<'a>(output: &mut String, heading: &str, values: impl Iterator<Item = &'a str>) {
    output.push_str("## ");
    output.push_str(heading);
    output.push_str("\n\n");
    let mut seen = Vec::<String>::new();
    for value in values.map(str::trim).filter(|value| !value.is_empty()) {
        if !seen.iter().any(|existing| existing == value) {
            output.push_str("- ");
            output.push_str(value);
            output.push('\n');
            seen.push(value.to_string());
        }
    }
    output.push('\n');
}

pub fn parse_checkpoint_records(content: &str) -> Vec<CheckpointRecord> {
    let Some((_, checkpoint_text)) = content.split_once(CHECKPOINT_HEADING) else {
        return Vec::new();
    };
    let mut records = Vec::new();
    let mut remaining = checkpoint_text;
    while let Some(heading_pos) = remaining.find("\n### ") {
        remaining = &remaining[heading_pos + 1..];
        let Some(fence_start) = remaining.find("```json\n") else {
            break;
        };
        let json_start = fence_start + "```json\n".len();
        let Some(fence_end) = remaining[json_start..].find("\n```") else {
            break;
        };
        let json_text = &remaining[json_start..json_start + fence_end];
        if let Ok(record) = serde_json::from_str::<CheckpointRecord>(json_text) {
            records.push(record);
        }
        remaining = &remaining[json_start + fence_end + "\n```".len()..];
    }
    records
}

pub fn summary(content: &str) -> String {
    const SECTIONS: &[&str] = &[
        "用户核心目标",
        "已确认事实",
        "已完成修改",
        "关键设计决定",
        "测试结果",
        "当前运行状态",
        "剩余问题",
        "下一步",
    ];
    let mut parts = Vec::new();
    for section in SECTIONS {
        if let Some(body) = section_body(content, section) {
            let compact = body
                .lines()
                .map(str::trim)
                .filter(|line| !line.is_empty())
                .collect::<Vec<_>>()
                .join(" ");
            if !compact.is_empty() {
                parts.push(format!("{section}: {compact}"));
            }
        }
    }
    if parts.is_empty() {
        "未记录结构化摘要".to_string()
    } else {
        parts.join("；")
    }
}

fn section_body<'a>(content: &'a str, heading: &str) -> Option<&'a str> {
    let marker = format!("## {heading}");
    let start = content.find(&marker)? + marker.len();
    let tail = &content[start..];
    let end = tail.find("\n## ").unwrap_or(tail.len());
    Some(tail[..end].trim())
}

pub fn checkpoint_from_args(
    args: &Value,
    default_timestamp: &str,
) -> Result<CheckpointRecord, String> {
    let explicit_turn_id = args
        .get("turn_id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let explicit_timestamp = string_field(args, "timestamp");
    let mut record = CheckpointRecord {
        turn_id: explicit_turn_id.unwrap_or_default().to_string(),
        timestamp: explicit_timestamp.clone().unwrap_or_default(),
        user_intent: string_field(args, "user_intent").unwrap_or_default(),
        findings: string_array(args, "findings")?,
        decisions: string_array(args, "decisions")?,
        files_changed: string_array(args, "files_changed")?,
        tests: string_array(args, "tests")?,
        runtime_state: string_array(args, "runtime_state")?,
        remaining_issues: string_array(args, "remaining_issues")?,
        next_actions: string_array(args, "next_actions")?,
        notes: string_field(args, "notes").unwrap_or_default(),
    };
    if record.turn_id.is_empty() {
        record.turn_id = automatic_turn_id(&record);
    }
    record.timestamp = explicit_timestamp.unwrap_or_else(|| default_timestamp.to_string());
    Ok(record)
}

fn automatic_turn_id(record: &CheckpointRecord) -> String {
    let encoded = serde_json::to_vec(record).expect("checkpoint record is serializable");
    let hash = format!("{:x}", Sha256::digest(encoded));
    format!("auto-{}", &hash[..16])
}

fn string_field(args: &Value, name: &str) -> Option<String> {
    args.get(name).and_then(Value::as_str).map(str::to_string)
}

fn string_array(args: &Value, name: &str) -> Result<Vec<String>, String> {
    let Some(value) = args.get(name) else {
        return Ok(Vec::new());
    };
    let array = value
        .as_array()
        .ok_or_else(|| format!("{name} must be an array of strings"))?;
    array
        .iter()
        .map(|item| {
            item.as_str()
                .map(str::to_string)
                .ok_or_else(|| format!("{name} must contain only strings"))
        })
        .collect()
}

pub fn redact_record(record: &mut CheckpointRecord) -> bool {
    let mut changed = redact_text(&mut record.timestamp);
    changed |= redact_text(&mut record.user_intent);
    changed |= redact_text(&mut record.notes);
    for values in [
        &mut record.findings,
        &mut record.decisions,
        &mut record.files_changed,
        &mut record.tests,
        &mut record.runtime_state,
        &mut record.remaining_issues,
        &mut record.next_actions,
    ] {
        for value in values {
            changed |= redact_text(value);
        }
    }
    changed
}

fn redact_text(value: &mut String) -> bool {
    static PATTERNS: OnceLock<Vec<Regex>> = OnceLock::new();
    let patterns = PATTERNS.get_or_init(|| {
        vec![
            Regex::new(r"(?i)\b(bearer\s+)[A-Za-z0-9._~+/=-]{6,}").expect("bearer regex"),
            Regex::new(r"(?i)\b(api[_ -]?key|token|cookie|password|passwd|pwd)\s*[:=]\s*[^\s,;]+")
                .expect("secret assignment regex"),
            Regex::new(r"(?is)-----BEGIN[^\n]*PRIVATE KEY-----.*?-----END[^\n]*PRIVATE KEY-----")
                .expect("private key regex"),
        ]
    });
    let original = value.clone();
    let mut redacted = value.clone();
    redacted = patterns[0]
        .replace_all(&redacted, "${1}[REDACTED]")
        .into_owned();
    redacted = patterns[1]
        .replace_all(&redacted, |captures: &regex::Captures<'_>| {
            format!("{}=[REDACTED]", &captures[1])
        })
        .into_owned();
    redacted = patterns[2]
        .replace_all(&redacted, "[REDACTED]")
        .into_owned();
    *value = redacted;
    *value != original
}
