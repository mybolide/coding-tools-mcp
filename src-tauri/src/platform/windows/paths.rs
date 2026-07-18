use std::path::PathBuf;

use windows::Win32::UI::Shell::{FOLDERID_RoamingAppData, SHGetKnownFolderPath, KF_FLAG_DEFAULT};

use crate::error::{AppError, AppResult};
use crate::platform::paths::{append_if_exists, resolve_from_path};

pub fn roaming_app_data() -> AppResult<PathBuf> {
    unsafe {
        let raw = SHGetKnownFolderPath(&FOLDERID_RoamingAppData, KF_FLAG_DEFAULT, None)
            .map_err(|err| AppError::Message(format!("SHGetKnownFolderPath failed: {err}")))?;
        let path = raw
            .to_string()
            .map_err(|err| AppError::Message(err.to_string()))?;
        Ok(PathBuf::from(path))
    }
}

pub fn cloudflared_candidates() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Some(found) = resolve_from_path("cloudflared") {
        paths.push(found);
    }
    append_if_exists(&mut paths, r"C:\Program Files\cloudflared\cloudflared.exe");
    append_if_exists(
        &mut paths,
        r"C:\Program Files (x86)\cloudflared\cloudflared.exe",
    );
    if let Some(home) = dirs::home_dir() {
        append_if_exists(
            &mut paths,
            home.join(".cloudflared").join("cloudflared.exe"),
        );
    }
    paths
}

pub fn frpc_candidates() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Some(found) = resolve_from_path("frpc") {
        paths.push(found);
    }
    append_if_exists(&mut paths, r"C:\Program Files\frp\frpc.exe");
    if let Some(home) = dirs::home_dir() {
        append_if_exists(&mut paths, home.join(".frp").join("frpc.exe"));
    }
    paths
}
