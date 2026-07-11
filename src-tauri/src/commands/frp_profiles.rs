use tauri::State;

use crate::app_state::AppState;
use crate::error::{AppError, AppResult};
use crate::secret::SecretStore;
use crate::settings::{AppSettings, FrpProfile};

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FrpProfileDto {
    pub id: String,
    pub name: String,
    pub server: String,
    pub server_port: u16,
    pub has_token: bool,
}

#[tauri::command]
pub fn list_frp_profiles(state: State<'_, AppState>) -> AppResult<Vec<FrpProfileDto>> {
    state.with_settings(|store| {
        Ok(store
            .get()
            .frp_profiles
            .iter()
            .map(|profile| {
                let has_token = SecretStore::get_app("frp_profile_token", &profile.id)
                    .ok()
                    .flatten()
                    .is_some_and(|value| !value.trim().is_empty());
                FrpProfileDto {
                    id: profile.id.clone(),
                    name: profile.name.clone(),
                    server: profile.server.clone(),
                    server_port: profile.server_port,
                    has_token,
                }
            })
            .collect())
    })
}

#[tauri::command]
pub fn save_frp_profile(
    state: State<'_, AppState>,
    profile: FrpProfile,
    token: Option<String>,
) -> AppResult<FrpProfileDto> {
    if profile.name.trim().is_empty() || profile.server.trim().is_empty() {
        return Err(AppError::Message("FRP 配置名称和服务器不能为空。".into()));
    }
    let mut saved = profile;
    saved.name = saved.name.trim().to_string();
    saved.server = saved.server.trim().to_string();
    if saved.id.trim().is_empty() {
        saved.id = uuid::Uuid::new_v4().to_string().replace('-', "");
    }

    state.with_settings(|store| {
        let mut settings = store.get().clone();
        if let Some(existing) = settings
            .frp_profiles
            .iter_mut()
            .find(|item| item.id == saved.id)
        {
            *existing = saved.clone();
        } else {
            settings.frp_profiles.push(saved.clone());
        }
        store.update(settings)?;
        Ok(())
    })?;

    if let Some(token) = token.filter(|value| !value.trim().is_empty()) {
        SecretStore::set_app("frp_profile_token", &saved.id, token.trim())?;
    }

    Ok(FrpProfileDto {
        id: saved.id.clone(),
        name: saved.name,
        server: saved.server,
        server_port: saved.server_port,
        has_token: SecretStore::get_app("frp_profile_token", &saved.id)?
            .is_some_and(|value| !value.trim().is_empty()),
    })
}

#[tauri::command]
pub fn delete_frp_profile(state: State<'_, AppState>, id: String) -> AppResult<()> {
    state.with_settings(|store| {
        let mut settings = store.get().clone();
        settings.frp_profiles.retain(|profile| profile.id != id);
        store.update(settings)
    })?;
    let _ = SecretStore::delete_app("frp_profile_token", &id);
    Ok(())
}

#[tauri::command]
pub fn get_app_settings(state: State<'_, AppState>) -> AppResult<AppSettings> {
    state.with_settings(|store| Ok(store.get().clone()))
}

#[tauri::command]
pub fn set_last_workspace(state: State<'_, AppState>, id: String) -> AppResult<()> {
    state.with_settings(|store| {
        let mut settings = store.get().clone();
        settings.last_workspace_id = id;
        store.update(settings)
    })
}

#[tauri::command]
pub fn get_last_workspace_id(state: State<'_, AppState>) -> AppResult<String> {
    state.with_settings(|store| Ok(store.get().last_workspace_id.clone()))
}
