use std::fs;
use std::path::PathBuf;

use crate::error::AppResult;
use crate::platform::platform;

use super::model::AppSettings;

const SETTINGS_FILE: &str = "app_settings.json";

fn settings_path() -> AppResult<std::path::PathBuf> {
    Ok(platform().app_config_dir()?.join(SETTINGS_FILE))
}

#[derive(Debug)]
pub struct AppSettingsStore {
    path: PathBuf,
    settings: AppSettings,
}

impl AppSettingsStore {
    pub fn load() -> AppResult<Self> {
        let path = settings_path()?;
        let settings = if path.exists() {
            let raw = fs::read_to_string(&path)?;
            serde_json::from_str(&raw).unwrap_or_default()
        } else {
            AppSettings::default()
        };
        Ok(Self { path, settings })
    }

    pub fn get(&self) -> &AppSettings {
        &self.settings
    }

    pub fn update(&mut self, settings: AppSettings) -> AppResult<()> {
        self.settings = settings;
        self.save()
    }

    fn save(&self) -> AppResult<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        let text = serde_json::to_string_pretty(&self.settings)?;
        fs::write(&self.path, format!("{text}\n"))?;
        Ok(())
    }
}
