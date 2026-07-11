use std::sync::Mutex;

use crate::error::AppResult;
use crate::runtime::RuntimeSupervisor;
use crate::secret::SecretStore;
use crate::settings::AppSettingsStore;
use crate::workspace::WorkspaceStore;

pub struct AppState {
    pub workspaces: Mutex<WorkspaceStore>,
    pub runtime: Mutex<RuntimeSupervisor>,
    pub settings: Mutex<AppSettingsStore>,
}

impl AppState {
    pub fn new() -> AppResult<Self> {
        Ok(Self {
            workspaces: Mutex::new(WorkspaceStore::load()?),
            runtime: Mutex::new(RuntimeSupervisor::default()),
            settings: Mutex::new(AppSettingsStore::load()?),
        })
    }

    pub fn with_workspaces<R>(&self, f: impl FnOnce(&mut WorkspaceStore) -> AppResult<R>) -> AppResult<R> {
        let mut guard = self
            .workspaces
            .lock()
            .map_err(|_| crate::error::AppError::Message("workspace store poisoned".into()))?;
        f(&mut guard)
    }

    pub fn with_runtime<R>(&self, f: impl FnOnce(&mut RuntimeSupervisor) -> AppResult<R>) -> AppResult<R> {
        let mut guard = self
            .runtime
            .lock()
            .map_err(|_| crate::error::AppError::Message("runtime supervisor poisoned".into()))?;
        f(&mut guard)
    }

    pub fn with_settings<R>(&self, f: impl FnOnce(&mut AppSettingsStore) -> AppResult<R>) -> AppResult<R> {
        let mut guard = self
            .settings
            .lock()
            .map_err(|_| crate::error::AppError::Message("app settings poisoned".into()))?;
        f(&mut guard)
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new().expect("failed to initialize app state")
    }
}

pub fn bootstrap_workspace(profile_id: &str) -> AppResult<()> {
    SecretStore::init_workspace_secrets(profile_id)
}

pub fn teardown_workspace(profile_id: &str) -> AppResult<()> {
    SecretStore::remove_workspace_secrets(profile_id)
}
