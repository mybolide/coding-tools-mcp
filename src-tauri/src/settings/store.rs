use crate::data::DataStore;
use crate::error::AppResult;

pub type AppSettingsStore = DataStore;

pub fn settings_path() -> AppResult<std::path::PathBuf> {
    crate::data::data_file_path()
}
