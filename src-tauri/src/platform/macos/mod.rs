mod net;
mod paths;
mod process;

use std::path::PathBuf;

use crate::error::AppResult;
use crate::platform::paths as shared_paths;
use crate::platform::Platform;

pub struct MacPlatform;

impl Platform for MacPlatform {
    fn os_name(&self) -> &'static str {
        "macos"
    }

    fn app_config_dir(&self) -> AppResult<PathBuf> {
        let base = dirs::home_dir()
            .ok_or_else(|| crate::error::AppError::Message("home dir not found".into()))?;
        Ok(base
            .join("Library")
            .join("Application Support")
            .join("coding-tools-mcp-desktop"))
    }

    fn find_pid_listening_on_port(&self, port: u16) -> AppResult<Option<u32>> {
        net::find_pid_listening_on_port(port)
    }

    fn process_image_path(&self, pid: u32) -> AppResult<Option<String>> {
        process::process_image_path(pid)
    }

    fn is_process_alive(&self, pid: u32) -> bool {
        process::is_process_alive(pid)
    }

    fn terminate_process_tree(&self, pid: u32) -> AppResult<()> {
        process::terminate_process_tree(pid)
    }

    fn resolve_executable(&self, name: &str) -> Option<PathBuf> {
        shared_paths::resolve_from_path(name)
    }

    fn cloudflared_candidates(&self) -> Vec<PathBuf> {
        paths::cloudflared_candidates()
    }

    fn frpc_candidates(&self) -> Vec<PathBuf> {
        paths::frpc_candidates()
    }
}
