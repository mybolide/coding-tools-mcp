use std::path::Path;
use std::process::Command;

use crate::error::{AppError, AppResult};

pub fn open_path_in_file_manager(path: &Path) -> AppResult<()> {
    if !path.is_dir() {
        return Err(AppError::Message(format!(
            "路径不存在或不是目录: {}",
            path.display()
        )));
    }

    #[cfg(target_os = "windows")]
    {
        Command::new("explorer")
            .arg(path)
            .spawn()
            .map_err(|err| AppError::Message(format!("无法打开目录: {err}")))
            .map(|_| ())
    }

    #[cfg(target_os = "macos")]
    {
        Command::new("open")
            .arg(path)
            .spawn()
            .map_err(|err| AppError::Message(format!("无法打开目录: {err}")))
            .map(|_| ())
    }

    #[cfg(target_os = "linux")]
    {
        Command::new("xdg-open")
            .arg(path)
            .spawn()
            .map_err(|err| AppError::Message(format!("无法打开目录: {err}")))
            .map(|_| ())
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        let _ = path;
        Err(AppError::Message("当前平台不支持打开目录。".into()))
    }
}
