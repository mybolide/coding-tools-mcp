use std::fs;
use std::path::PathBuf;

use crate::error::{AppError, AppResult};
use crate::platform::platform;
use super::legacy_import::import_legacy_profiles_if_empty;
use super::model::WorkspaceProfile;

const PROFILES_FILE: &str = "profiles.json";

#[derive(Debug)]
pub struct WorkspaceStore {
    profiles_path: PathBuf,
    profiles: Vec<WorkspaceProfile>,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct ProfilesFile {
    profiles: Vec<WorkspaceProfile>,
}

impl WorkspaceStore {
    pub fn load() -> AppResult<Self> {
        let profiles_path = app_home()?.join(PROFILES_FILE);
        let mut profiles = if profiles_path.exists() {
            let raw = fs::read_to_string(&profiles_path)?;
            serde_json::from_str::<ProfilesFile>(&raw)?.profiles
        } else {
            Vec::new()
        };
        let imported = import_legacy_profiles_if_empty(&mut profiles)?;
        let store = Self {
            profiles_path,
            profiles,
        };
        if imported > 0 {
            store.save()?;
        }
        Ok(store)
    }

    pub fn list(&self) -> &[WorkspaceProfile] {
        &self.profiles
    }

    pub fn get(&self, id: &str) -> Option<&WorkspaceProfile> {
        self.profiles.iter().find(|p| p.id == id)
    }

    #[allow(dead_code)]
    pub fn get_mut(&mut self, id: &str) -> Option<&mut WorkspaceProfile> {
        self.profiles.iter_mut().find(|p| p.id == id)
    }

    pub fn add(&mut self, profile: WorkspaceProfile) -> AppResult<()> {
        self.profiles.push(profile);
        self.save()
    }

    pub fn update(&mut self, profile: WorkspaceProfile) -> AppResult<()> {
        let Some(index) = self.profiles.iter().position(|p| p.id == profile.id) else {
            return Err(AppError::Message(format!(
                "workspace not found: {}",
                profile.id
            )));
        };
        self.profiles[index] = profile;
        self.save()
    }

    pub fn remove(&mut self, id: &str) -> AppResult<Option<WorkspaceProfile>> {
        let Some(index) = self.profiles.iter().position(|p| p.id == id) else {
            return Ok(None);
        };
        let removed = self.profiles.remove(index);
        self.save()?;
        Ok(Some(removed))
    }

    fn save(&self) -> AppResult<()> {
        if let Some(parent) = self.profiles_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let payload = ProfilesFile {
            profiles: self.profiles.clone(),
        };
        let text = serde_json::to_string_pretty(&payload)?;
        fs::write(&self.profiles_path, format!("{text}\n"))?;
        Ok(())
    }
}

pub fn app_home() -> AppResult<PathBuf> {
    platform().app_config_dir()
}
