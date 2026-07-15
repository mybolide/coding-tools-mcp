use std::path::PathBuf;

use tauri::State;

use crate::app_state::{bootstrap_workspace, teardown_workspace, AppState};
use crate::error::{AppError, AppResult};
use crate::platform::open_path_in_file_manager;
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
        bootstrap_workspace(store, &profile.id)?;
        store.add(profile.clone())?;
        Ok(profile)
    })
}

#[tauri::command]
pub fn update_workspace(state: State<'_, AppState>, profile: WorkspaceProfile) -> AppResult<()> {
    state.with_workspaces(|store| {
        validate_unique_frp_subdomains(store.list(), &profile)?;
        store.update(profile)
    })
}

#[tauri::command]
pub fn open_workspace_directory(path: String) -> AppResult<()> {
    let path = PathBuf::from(path.trim());
    open_path_in_file_manager(&path)
}

#[tauri::command]
pub fn delete_workspace(state: State<'_, AppState>, id: String) -> AppResult<()> {
    let profile = state.with_workspaces(|store| {
        store
            .get(&id)
            .cloned()
            .ok_or_else(|| AppError::Message(format!("workspace not found: {id}")))
    })?;
    tauri::async_runtime::block_on(drop_tunnel_workspace(&id))?;
    state.with_runtime(|runtime| {
        runtime.drop_workspace(&profile);
        Ok(())
    })?;
    state.with_workspaces(|store| {
        if store.remove(&id)?.is_some() {
            teardown_workspace(store, &id)?;
        }
        Ok(())
    })
}

fn validate_unique_frp_subdomains(
    profiles: &[WorkspaceProfile],
    candidate: &WorkspaceProfile,
) -> AppResult<()> {
    let mut claims = std::collections::HashMap::<String, (String, String)>::new();

    for profile in profiles.iter().filter(|profile| profile.id != candidate.id) {
        for (service, subdomain) in frp_subdomain_claims(profile) {
            claims
                .entry(subdomain.to_ascii_lowercase())
                .or_insert_with(|| (profile.name.clone(), service.to_string()));
        }
    }

    let mut candidate_claims = std::collections::HashMap::<String, String>::new();
    for (service, subdomain) in frp_subdomain_claims(candidate) {
        let normalized = subdomain.to_ascii_lowercase();
        if let Some((owner_name, owner_service)) = claims.get(&normalized) {
            return Err(AppError::Message(format!(
                "FRP 子域名“{}”已被工作区“{}”的 {} 服务使用，不能重复。",
                subdomain, owner_name, owner_service
            )));
        }
        if let Some(owner_service) = candidate_claims.get(&normalized) {
            return Err(AppError::Message(format!(
                "FRP 子域名“{}”同时用于当前工作区的 {} 和 {} 服务，不能重复。",
                subdomain, owner_service, service
            )));
        }
        candidate_claims.insert(normalized, service.to_string());
    }

    Ok(())
}

fn frp_subdomain_claims(profile: &WorkspaceProfile) -> Vec<(&'static str, &str)> {
    [
        (
            "MCP",
            profile.tunnel.tunnel_type.as_str(),
            profile.tunnel.frp_subdomain.as_str(),
        ),
        (
            "Actions",
            profile.actions.tunnel_type.as_str(),
            profile.actions.frp_subdomain.as_str(),
        ),
    ]
    .into_iter()
    .filter_map(|(service, tunnel_type, subdomain)| {
        let subdomain = subdomain.trim();
        (tunnel_type == "frp" && !subdomain.is_empty()).then_some((service, subdomain))
    })
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frp_profile(name: &str, mcp_subdomain: &str, actions_subdomain: &str) -> WorkspaceProfile {
        let mut profile = WorkspaceProfile::new(format!("C:/workspace/{name}"), Some(name.into()));
        profile.tunnel.tunnel_type = "frp".into();
        profile.tunnel.frp_subdomain = mcp_subdomain.into();
        profile.actions.tunnel_type = "frp".into();
        profile.actions.frp_subdomain = actions_subdomain.into();
        profile
    }

    #[test]
    fn rejects_duplicate_subdomains_across_workspaces() {
        let first = frp_profile("first", "shared", "first-actions");
        let second = frp_profile("second", "SHARED", "second-actions");
        let error = validate_unique_frp_subdomains(&[first], &second).unwrap_err();
        assert!(error.to_string().contains("不能重复"));
    }

    #[test]
    fn rejects_duplicate_subdomains_between_services() {
        let profile = frp_profile("demo", "same", "same");
        let error = validate_unique_frp_subdomains(&[], &profile).unwrap_err();
        assert!(error.to_string().contains("不能重复"));
    }

    #[test]
    fn allows_replacing_a_workspace_own_subdomain() {
        let original = frp_profile("demo", "a", "demo-actions");
        let mut updated = original.clone();
        updated.tunnel.frp_subdomain = "aa".into();
        assert!(validate_unique_frp_subdomains(&[original], &updated).is_ok());
    }

    #[test]
    fn legacy_duplicates_do_not_block_an_unrelated_workspace_update() {
        let first = frp_profile("first", "legacy", "first-actions");
        let second = frp_profile("second", "LEGACY", "second-actions");
        let candidate = frp_profile("candidate", "candidate", "candidate-actions");
        assert!(validate_unique_frp_subdomains(&[first, second], &candidate).is_ok());
    }
}
