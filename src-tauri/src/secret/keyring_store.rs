use crate::error::AppResult;

const SERVICE: &str = "coding-tools-mcp-desktop";

/// Keys that can be shared across workspaces. When a workspace opts into shared
/// secrets, each key defaults to the shared value instead of the per-workspace
/// one. The set mirrors `init_workspace_secrets`.
const SHARED_KEYS: &[&str] = &[
    "bearer_token",
    "oauth_client_secret",
    "oauth_password",
    "oauth_token_secret",
    "actions_api_key",
    "actions_oauth_client_secret",
    "actions_oauth_password",
    "actions_oauth_token_secret",
];

pub struct SecretStore;

impl SecretStore {
    pub fn init_workspace_secrets(profile_id: &str) -> AppResult<()> {
        Self::set(profile_id, "oauth_client_secret", &random_secret())?;
        Self::set(profile_id, "oauth_password", &random_secret())?;
        Self::set(profile_id, "oauth_token_secret", &random_secret())?;
        Self::set(profile_id, "bearer_token", &random_secret())?;
        Self::set(profile_id, "actions_api_key", &random_secret())?;
        Self::set(profile_id, "actions_oauth_client_secret", &random_secret())?;
        Self::set(profile_id, "actions_oauth_password", &random_secret())?;
        Self::set(profile_id, "actions_oauth_token_secret", &random_secret())?;
        Ok(())
    }

    pub fn remove_workspace_secrets(profile_id: &str) -> AppResult<()> {
        for key in [
            "oauth_client_secret",
            "oauth_password",
            "oauth_token_secret",
            "bearer_token",
            "actions_api_key",
            "actions_oauth_client_secret",
            "actions_oauth_password",
            "actions_oauth_token_secret",
            "cloudflare_token",
            "actions_cloudflare_token",
            "frp_token",
            "actions_frp_token",
        ] {
            let _ = Self::delete(profile_id, key);
        }
        Ok(())
    }

    pub fn set(profile_id: &str, key: &str, value: &str) -> AppResult<()> {
        entry(profile_id, key)?.set_password(value)?;
        Ok(())
    }

    pub fn get(profile_id: &str, key: &str) -> AppResult<Option<String>> {
        match entry(profile_id, key)?.get_password() {
            Ok(value) => Ok(Some(value)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(err) => Err(err.into()),
        }
    }

    pub fn regenerate(profile_id: &str, key: &str) -> AppResult<String> {
        let value = random_secret();
        Self::set(profile_id, key, &value)?;
        Ok(value)
    }

    /// Seed all shared secrets with random values (idempotent — skips existing).
    pub fn init_shared_secrets() -> AppResult<()> {
        let mut settings = crate::settings::AppSettings::load_or_default();
        let mut changed = false;
        for key in SHARED_KEYS {
            if !settings.shared_secrets.contains_key(*key) {
                settings
                    .shared_secrets
                    .insert(key.to_string(), random_secret());
                changed = true;
            }
        }
        if changed {
            settings.save()?;
        }
        Ok(())
    }

    /// Read a shared secret from the app settings JSON. Returns None if never set.
    pub fn get_shared(key: &str) -> AppResult<Option<String>> {
        let settings = crate::settings::AppSettings::load_or_default();
        Ok(settings.shared_secrets.get(key).cloned())
    }

    /// Generate a new random shared secret, persist it in app settings, and return it.
    pub fn regenerate_shared(key: &str) -> AppResult<String> {
        let value = random_secret();
        let mut settings = crate::settings::AppSettings::load_or_default();
        settings.shared_secrets.insert(key.to_string(), value.clone());
        settings.save()?;
        Ok(value)
    }

    pub fn get_app(scope: &str, item_id: &str) -> AppResult<Option<String>> {
        match entry_app(scope, item_id)?.get_password() {
            Ok(value) => Ok(Some(value)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(err) => Err(err.into()),
        }
    }

    pub fn set_app(scope: &str, item_id: &str, value: &str) -> AppResult<()> {
        entry_app(scope, item_id)?.set_password(value)?;
        Ok(())
    }

    pub fn delete_app(scope: &str, item_id: &str) -> AppResult<()> {
        match entry_app(scope, item_id)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(err) => Err(err.into()),
        }
    }

    fn delete(profile_id: &str, key: &str) -> AppResult<()> {
        match entry(profile_id, key)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(err) => Err(err.into()),
        }
    }
}

fn entry(profile_id: &str, key: &str) -> AppResult<keyring::Entry> {
    let account = format!("{profile_id}:{key}");
    Ok(keyring::Entry::new(SERVICE, &account)?)
}

fn entry_app(scope: &str, item_id: &str) -> AppResult<keyring::Entry> {
    let account = format!("app:{scope}:{item_id}");
    Ok(keyring::Entry::new(SERVICE, &account)?)
}

fn random_secret() -> String {
    format!("{}{}", uuid::Uuid::new_v4(), uuid::Uuid::new_v4()).replace('-', "")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn random_secret_is_non_empty() {
        assert!(random_secret().len() > 32);
    }
}
