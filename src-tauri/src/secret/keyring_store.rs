use crate::data::DataStore;
use crate::error::AppResult;

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
        DataStore::update_file(|data| {
            let store = workspace_secret_map(data, profile_id);
            store.insert("oauth_password".into(), random_secret());
            store.insert("oauth_token_secret".into(), random_secret());
            store.insert("bearer_token".into(), random_secret());
            store.insert("actions_api_key".into(), random_secret());
            store.insert("actions_oauth_client_secret".into(), random_secret());
            store.insert("actions_oauth_password".into(), random_secret());
            store.insert("actions_oauth_token_secret".into(), random_secret());
            Ok(())
        })
    }

    pub fn remove_workspace_secrets(profile_id: &str) -> AppResult<()> {
        DataStore::update_file(|data| {
            data.workspace_secrets.remove(profile_id);
            Ok(())
        })
    }

    pub fn set(profile_id: &str, key: &str, value: &str) -> AppResult<()> {
        DataStore::update_file(|data| {
            workspace_secret_map(data, profile_id).insert(key.to_string(), value.to_string());
            Ok(())
        })
    }

    pub fn get(profile_id: &str, key: &str) -> AppResult<Option<String>> {
        DataStore::read_file(|data| {
            Ok(data
                .workspace_secrets
                .get(profile_id)
                .and_then(|secrets| secrets.get(key))
                .filter(|value| !value.is_empty())
                .cloned())
        })
    }

    pub fn regenerate(profile_id: &str, key: &str) -> AppResult<String> {
        let value = random_secret();
        Self::set(profile_id, key, &value)?;
        Ok(value)
    }

    pub fn init_shared_secrets() -> AppResult<()> {
        DataStore::update_file(|data| {
            for key in SHARED_KEYS {
                if !data.shared_secrets.contains_key(*key) {
                    data.shared_secrets.insert(key.to_string(), random_secret());
                }
            }
            Ok(())
        })
    }

    pub fn get_shared(key: &str) -> AppResult<Option<String>> {
        DataStore::read_file(|data| Ok(data.shared_secrets.get(key).cloned()))
    }

    pub fn regenerate_shared(key: &str) -> AppResult<String> {
        let value = random_secret();
        DataStore::update_file(|data| {
            data.shared_secrets.insert(key.to_string(), value.clone());
            Ok(())
        })?;
        Ok(value)
    }

    pub fn get_app(scope: &str, item_id: &str) -> AppResult<Option<String>> {
        DataStore::read_file(|data| {
            Ok(data
                .app_secrets
                .get(scope)
                .and_then(|items| items.get(item_id))
                .filter(|value| !value.is_empty())
                .cloned())
        })
    }

    pub fn set_app(scope: &str, item_id: &str, value: &str) -> AppResult<()> {
        DataStore::update_file(|data| {
            data.app_secrets
                .entry(scope.to_string())
                .or_default()
                .insert(item_id.to_string(), value.to_string());
            Ok(())
        })
    }

    pub fn delete_app(scope: &str, item_id: &str) -> AppResult<()> {
        DataStore::update_file(|data| {
            if let Some(items) = data.app_secrets.get_mut(scope) {
                items.remove(item_id);
                if items.is_empty() {
                    data.app_secrets.remove(scope);
                }
            }
            Ok(())
        })
    }
}

fn workspace_secret_map<'a>(
    data: &'a mut crate::data::AppData,
    profile_id: &str,
) -> &'a mut std::collections::HashMap<String, String> {
    data.workspace_secrets
        .entry(profile_id.to_string())
        .or_default()
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

    #[test]
    fn workspace_secret_roundtrip() {
        let id = uuid::Uuid::new_v4().to_string().replace('-', "");
        SecretStore::set(&id, "oauth_client_secret", "roundtrip-secret").expect("set");
        let loaded = SecretStore::get(&id, "oauth_client_secret").expect("get");
        assert_eq!(loaded.as_deref(), Some("roundtrip-secret"));
        let _ = SecretStore::remove_workspace_secrets(&id);
    }
}
