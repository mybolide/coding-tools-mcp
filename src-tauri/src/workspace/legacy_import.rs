use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::data::AppData;
use crate::error::AppResult;
use crate::workspace::model::WorkspaceProfile;

#[derive(Deserialize)]
struct LegacyProfilesFile {
    profiles: Vec<WorkspaceProfile>,
}

pub fn legacy_app_home() -> Option<PathBuf> {
    dirs::home_dir().map(|home| home.join(".coding-tools-mcp-desktop"))
}

pub fn import_legacy_profiles_if_empty(data: &mut AppData) -> AppResult<usize> {
    if !data.profiles.is_empty() {
        return Ok(0);
    }
    let Some(legacy_home) = legacy_app_home() else {
        return Ok(0);
    };
    let profiles_path = legacy_home.join("profiles.json");
    if !profiles_path.is_file() {
        return Ok(0);
    }
    let raw = fs::read_to_string(&profiles_path)?;
    let legacy: LegacyProfilesFile = serde_json::from_str(&raw)?;
    let secrets = load_legacy_secrets(&legacy_home)?;
    let mut imported = 0usize;
    for mut profile in legacy.profiles {
        if profile.path.trim().is_empty() || !PathBuf::from(&profile.path).exists() {
            continue;
        }
        migrate_legacy_secrets(data, &profile.id, secrets.get(&profile.id));
        normalize_legacy_profile(&mut profile);
        data.profiles.push(profile);
        imported += 1;
    }
    Ok(imported)
}

fn load_legacy_secrets(legacy_home: &Path) -> AppResult<HashMap<String, HashMap<String, String>>> {
    let secrets_path = legacy_home.join("secrets.json");
    if !secrets_path.is_file() {
        return Ok(HashMap::new());
    }
    let raw = fs::read_to_string(&secrets_path)?;
    Ok(serde_json::from_str(&raw).unwrap_or_default())
}

fn migrate_legacy_secrets(
    data: &mut AppData,
    profile_id: &str,
    secrets: Option<&HashMap<String, String>>,
) {
    let Some(secrets) = secrets else {
        return;
    };
    let mappings = [
        ("cloudflare_token", "cloudflare_token"),
        ("actions_cloudflare_token", "actions_cloudflare_token"),
        ("actions_api_key", "actions_api_key"),
        ("actions_oauth_client_secret", "actions_oauth_client_secret"),
        ("oauth_client_secret", "oauth_client_secret"),
        ("oauth_password", "oauth_password"),
        ("oauth_token_secret", "oauth_token_secret"),
        ("bearer_token", "bearer_token"),
    ];
    let store = data
        .workspace_secrets
        .entry(profile_id.to_string())
        .or_default();
    for (legacy_key, store_key) in mappings {
        if let Some(value) = secrets.get(legacy_key).filter(|value| !value.trim().is_empty()) {
            store.insert(store_key.to_string(), value.clone());
        }
    }
}

fn normalize_legacy_profile(profile: &mut WorkspaceProfile) {
    if profile.actions.local_port == 28766 {
        profile.actions.local_port = 8787;
    }
    profile.actions.cloudflare_token.clear();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_home_points_under_user_home() {
        let home = legacy_app_home();
        assert!(home.is_some());
        assert!(
            home.unwrap()
                .to_string_lossy()
                .contains(".coding-tools-mcp-desktop")
        );
    }
}
