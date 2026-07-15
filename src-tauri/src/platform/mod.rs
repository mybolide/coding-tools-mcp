use std::path::{Path, PathBuf};

use crate::error::AppResult;

/// Cross-platform OS primitives used by the desktop runtime.
///
/// Windows uses `windows-rs`. macOS and Linux live in dedicated modules.
#[allow(dead_code)]
pub trait Platform: Send + Sync {
    fn os_name(&self) -> &'static str;

    fn app_config_dir(&self) -> AppResult<PathBuf>;

    fn find_pid_listening_on_port(&self, port: u16) -> AppResult<Option<u32>>;

    /// Best-effort reclaim of a TCP listener on the given port. Windows uses
    /// `SetTcpEntry`; other platforms return `Ok(false)`.
    fn reclaim_listening_port(&self, port: u16) -> AppResult<bool> {
        let _ = port;
        Ok(false)
    }

    fn process_image_path(&self, pid: u32) -> AppResult<Option<String>>;

    fn is_process_alive(&self, pid: u32) -> bool;

    fn terminate_process_tree(&self, pid: u32) -> AppResult<()>;

    /// 清理由应用管理的同一路径进程；默认平台不做处理。
    fn terminate_processes_by_image_path(&self, _image_path: &Path) -> AppResult<usize> {
        Ok(0)
    }

    fn resolve_executable(&self, name: &str) -> Option<PathBuf>;

    fn cloudflared_candidates(&self) -> Vec<PathBuf>;

    fn frpc_candidates(&self) -> Vec<PathBuf>;
}

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;

mod open;
mod paths;

pub use open::open_path_in_file_manager;

#[cfg(target_os = "linux")]
pub use linux::LinuxPlatform;
#[cfg(target_os = "macos")]
pub use macos::MacPlatform;
#[cfg(target_os = "windows")]
pub use windows::WindowsPlatform;

static PLATFORM: std::sync::OnceLock<Box<dyn Platform>> = std::sync::OnceLock::new();

pub fn platform() -> &'static dyn Platform {
    PLATFORM.get_or_init(|| create_platform()).as_ref()
}

fn create_platform() -> Box<dyn Platform> {
    #[cfg(target_os = "windows")]
    {
        Box::new(WindowsPlatform)
    }
    #[cfg(target_os = "macos")]
    {
        Box::new(MacPlatform)
    }
    #[cfg(target_os = "linux")]
    {
        Box::new(LinuxPlatform)
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        struct Unsupported;
        impl Platform for Unsupported {
            fn os_name(&self) -> &'static str {
                "unsupported"
            }
            fn app_config_dir(&self) -> AppResult<PathBuf> {
                Err(crate::error::AppError::Message(
                    "unsupported operating system".into(),
                ))
            }
            fn find_pid_listening_on_port(&self, _port: u16) -> AppResult<Option<u32>> {
                Ok(None)
            }
            fn process_image_path(&self, _pid: u32) -> AppResult<Option<String>> {
                Ok(None)
            }
            fn is_process_alive(&self, _pid: u32) -> bool {
                false
            }
            fn terminate_process_tree(&self, _pid: u32) -> AppResult<()> {
                Ok(())
            }
            fn resolve_executable(&self, name: &str) -> Option<PathBuf> {
                paths::resolve_from_path(name)
            }
            fn cloudflared_candidates(&self) -> Vec<PathBuf> {
                paths::resolve_from_path("cloudflared")
                    .into_iter()
                    .collect()
            }
            fn frpc_candidates(&self) -> Vec<PathBuf> {
                paths::resolve_from_path("frpc").into_iter().collect()
            }
        }
        Box::new(Unsupported)
    }
}
