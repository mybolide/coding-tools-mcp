mod actions;
mod app_state;
mod auth;
mod commands;
mod data;
mod error;
pub mod harness;
mod health;
mod mcp;
mod platform;
mod runtime;
mod secret;
mod settings;
pub mod tools;
mod tunnel;
mod workspace;

use app_state::AppState;
use commands::{
    create_workspace, delete_frp_profile, delete_workspace, get_actions_runtime_status,
    get_app_settings, get_download_config, get_frp_snippet, get_last_workspace_id, get_proxy,
    get_runtime_status, get_shared_secret, get_workspace_secret, install_software,
    list_frp_profiles, list_software, list_workspaces, open_workspace_directory,
    read_workspace_logs, regenerate_shared_secret, regenerate_workspace_secret,
    restart_actions_runtime, restart_runtime, restart_tunnel, run_health_checks, save_frp_profile,
    set_download_config, set_last_workspace, set_proxy, set_shared_secret, set_workspace_secret,
    start_actions_runtime, start_runtime, start_tunnel, stop_actions_runtime, stop_runtime,
    stop_tunnel, test_tunnel, uninstall_software, update_workspace,
};
use tauri::Manager;

#[cfg(target_os = "windows")]
fn acquire_single_instance() -> bool {
    use windows::core::w;
    use windows::Win32::Foundation::{CloseHandle, GetLastError, ERROR_ALREADY_EXISTS};
    use windows::Win32::System::Threading::CreateMutexW;

    // 保持 mutex HANDLE 到进程退出，由 Windows 自动回收。第二个实例必须在
    // cleanup_managed_frpc_instances 之前退出，否则会清理第一个实例的 frpc。
    let Ok(handle) = (unsafe {
        CreateMutexW(
            None,
            false,
            w!("Local\\CodingToolsMcpDesktop-SingleInstance"),
        )
    }) else {
        eprintln!("创建应用单实例锁失败，为避免误清理其他实例的 frpc，本次启动已取消");
        return false;
    };
    if unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
        let _ = unsafe { CloseHandle(handle) };
        return false;
    }
    true
}

#[cfg(not(target_os = "windows"))]
fn acquire_single_instance() -> bool {
    true
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    if !acquire_single_instance() {
        return;
    }
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
            open_workspace_directory,
            delete_workspace,
            start_runtime,
            stop_runtime,
            get_runtime_status,
            start_actions_runtime,
            stop_actions_runtime,
            get_actions_runtime_status,
            restart_runtime,
            restart_actions_runtime,
            get_frp_snippet,
            start_tunnel,
            stop_tunnel,
            run_health_checks,
            get_workspace_secret,
            set_workspace_secret,
            regenerate_workspace_secret,
            get_shared_secret,
            set_shared_secret,
            regenerate_shared_secret,
            read_workspace_logs,
            list_frp_profiles,
            save_frp_profile,
            delete_frp_profile,
            get_app_settings,
            restart_tunnel,
            test_tunnel,
            set_last_workspace,
            get_last_workspace_id,
            list_software,
            install_software,
            uninstall_software,
            get_download_config,
            set_download_config,
            get_proxy,
            set_proxy,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
