use tauri::State;

use crate::app_state::AppState;
use crate::error::{AppError, AppResult};
use crate::health::{run_health_checks as execute_health_checks, HealthItem};

fn profile_by_id(state: &AppState, id: &str) -> AppResult<crate::workspace::WorkspaceProfile> {
    state.with_workspaces(|store| {
        store
            .get(id)
            .cloned()
            .ok_or_else(|| AppError::Message(format!("workspace not found: {id}")))
    })
}

#[tauri::command]
pub async fn run_health_checks(state: State<'_, AppState>, id: String) -> AppResult<Vec<HealthItem>> {
    let profile = profile_by_id(&state, &id)?;
    Ok(execute_health_checks(&profile).await)
}
