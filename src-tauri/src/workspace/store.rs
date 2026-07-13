use crate::data::DataStore;
use crate::error::AppResult;

pub type WorkspaceStore = DataStore;

pub fn app_home() -> AppResult<std::path::PathBuf> {
    crate::platform::platform().app_config_dir()
}
