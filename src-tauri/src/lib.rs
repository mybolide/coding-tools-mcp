mod settings;
mod actions;
mod auth;
mod app_state;
pub mod tools;
mod commands;
mod error;
mod health;
mod mcp;
mod platform;
mod runtime;
mod secret;
mod tunnel;
mod workspace;

use app_state::AppState;
use commands::{
    create_workspace, delete_frp_profile, delete_workspace, get_actions_runtime_status,
    get_app_settings, get_frp_snippet, get_last_workspace_id, get_runtime_status, get_workspace_secret,
    list_frp_profiles, list_workspaces, read_workspace_logs, regenerate_workspace_secret,
    restart_tunnel, run_health_checks, save_frp_profile, set_last_workspace, set_workspace_secret, start_actions_runtime,
    start_runtime, start_tunnel, stop_actions_runtime, stop_runtime, stop_tunnel, update_workspace,
};
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            app.manage(AppState::new().expect("failed to load app state"));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            list_workspaces,
            create_workspace,
            update_workspace,
            delete_workspace,
            start_runtime,
            stop_runtime,
            get_runtime_status,
            start_actions_runtime,
            stop_actions_runtime,
            get_actions_runtime_status,
            get_frp_snippet,
            start_tunnel,
            stop_tunnel,
            run_health_checks,
            get_workspace_secret,
            set_workspace_secret,
            regenerate_workspace_secret,
            read_workspace_logs,
            list_frp_profiles,
            save_frp_profile,
            delete_frp_profile,
            get_app_settings,
            restart_tunnel,
            set_last_workspace,
            get_last_workspace_id,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
