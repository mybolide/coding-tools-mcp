use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryIndex {
    pub version: u32,
    pub latest_number: u64,
    pub sessions: BTreeMap<String, IndexEntry>,
}

impl Default for HistoryIndex {
    fn default() -> Self {
        Self {
            version: 1,
            latest_number: 0,
            sessions: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexEntry {
    pub number: u64,
    pub path: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone)]
pub struct HistoryDocument {
    pub number: u64,
    pub path: String,
    pub content: String,
    pub session_key: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct ScanReport {
    pub documents: Vec<HistoryDocument>,
    pub numbers: Vec<u64>,
    pub missing_numbers: Vec<u64>,
    pub duplicate_session_keys: Vec<String>,
    pub invalid_files: Vec<String>,
    pub empty_files: Vec<String>,
}

impl ScanReport {
    pub fn latest_number(&self) -> Option<u64> {
        self.numbers.last().copied()
    }

    pub fn sequence_valid(&self) -> bool {
        self.missing_numbers.is_empty()
            && self.duplicate_session_keys.is_empty()
            && self.invalid_files.is_empty()
            && self.empty_files.is_empty()
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckpointRecord {
    pub turn_id: String,
    #[serde(default)]
    pub timestamp: String,
    #[serde(default)]
    pub user_intent: String,
    #[serde(default)]
    pub findings: Vec<String>,
    #[serde(default)]
    pub decisions: Vec<String>,
    #[serde(default)]
    pub files_changed: Vec<String>,
    #[serde(default)]
    pub tests: Vec<String>,
    #[serde(default)]
    pub runtime_state: Vec<String>,
    #[serde(default)]
    pub remaining_issues: Vec<String>,
    #[serde(default)]
    pub next_actions: Vec<String>,
    #[serde(default)]
    pub notes: String,
}
