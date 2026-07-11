use tauri::State;

use crate::app_state::{bootstrap_workspace, teardown_workspace, AppState};
use crate::error::AppResult;
use crate::tunnel::drop_workspace as drop_tunnel_workspace;
use crate::workspace::WorkspaceProfile;

#[tauri::command]
pub fn list_workspaces(state: State<'_, AppState>) -> AppResult<Vec<WorkspaceProfile>> {
    state.with_workspaces(|store| Ok(store.list().to_vec()))
}

#[tauri::command]
pub fn create_workspace(
    state: State<'_, AppState>,
    path: String,
    name: Option<String>,
) -> AppResult<WorkspaceProfile> {
    state.with_workspaces(|store| {
        let profile = WorkspaceProfile::new(path, name);
        bootstrap_workspace(&profile.id)?;
        store.add(profile.clone())?;
        Ok(profile)
    })
}

#[tauri::command]
pub fn update_workspace(state: State<'_, AppState>, profile: WorkspaceProfile) -> AppResult<()> {
    state.with_workspaces(|store| store.update(profile))
}

#[tauri::command]
pub fn delete_workspace(state: State<'_, AppState>, id: String) -> AppResult<()> {
    tauri::async_runtime::block_on(drop_tunnel_workspace(&id));
    state.with_runtime(|runtime| {
        runtime.drop_workspace(&id);
        Ok(())
    })?;
    state.with_workspaces(|store| {
        if store.remove(&id)?.is_some() {
            teardown_workspace(&id)?;
        }
        Ok(())
    })
}
