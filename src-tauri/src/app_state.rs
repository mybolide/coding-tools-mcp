use std::sync::Mutex;

use crate::data::DataStore;
use crate::error::AppResult;
use crate::runtime::RuntimeSupervisor;

pub struct AppState {
    pub data: Mutex<DataStore>,
    pub runtime: Mutex<RuntimeSupervisor>,
}

impl AppState {
    pub fn new() -> AppResult<Self> {
        let mut store = DataStore::load()?;
        store.init_shared_secrets()?;
        Ok(Self {
            data: Mutex::new(store),
            runtime: Mutex::new(RuntimeSupervisor::default()),
        })
    }

    pub fn with_data<R>(&self, f: impl FnOnce(&mut DataStore) -> AppResult<R>) -> AppResult<R> {
        let mut guard = self
            .data
            .lock()
            .map_err(|_| crate::error::AppError::Message("data store poisoned".into()))?;
        f(&mut guard)
    }

    pub fn with_workspaces<R>(
        &self,
        f: impl FnOnce(&mut DataStore) -> AppResult<R>,
    ) -> AppResult<R> {
        self.with_data(f)
    }

    pub fn with_settings<R>(&self, f: impl FnOnce(&mut DataStore) -> AppResult<R>) -> AppResult<R> {
        self.with_data(f)
    }

    pub fn with_runtime<R>(
        &self,
        f: impl FnOnce(&mut RuntimeSupervisor) -> AppResult<R>,
    ) -> AppResult<R> {
        let mut guard = self
            .runtime
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        f(&mut guard)
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new().expect("failed to initialize app state")
    }
}

pub fn bootstrap_workspace(store: &mut DataStore, profile_id: &str) -> AppResult<()> {
    store.init_workspace_secrets(profile_id)
}

pub fn teardown_workspace(store: &mut DataStore, profile_id: &str) -> AppResult<()> {
    store.remove_workspace_secrets(profile_id)
}
