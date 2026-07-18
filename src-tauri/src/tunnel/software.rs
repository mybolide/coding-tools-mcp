//! Software management for the two tunnel binaries: frpc and cloudflared.
//!
//! Both can be installed into the app cache `bin/` directory (downloaded from
//! GitHub, honoring the mirror + proxy config). Binaries found in the cache dir
//! are "managed" (uninstallable); binaries found on PATH or in system install
//! locations are reported but cannot be removed from here.

use std::path::PathBuf;

use serde::Serialize;

use crate::error::{AppError, AppResult};
use crate::platform::platform;
use crate::tunnel::cloudflare::resolve_cloudflared;
use crate::tunnel::cloudflare::{cached_cloudflared_path, download_cloudflared_to_cache};
use crate::tunnel::frp::{cached_frpc_path, download_frpc_to_cache, resolve_frpc};

/// Status of a managed tunnel binary, serialized to the frontend.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SoftwareStatus {
    /// "frpc" | "cloudflared"
    pub kind: String,
    /// Human-facing display name.
    pub name: String,
    /// Whether the binary was found anywhere (cache, PATH, or system dir).
    pub installed: bool,
    /// Resolved path if found.
    pub path: String,
    /// True when the resolved binary lives in the app cache dir (uninstallable).
    pub managed: bool,
}

fn frpc_status() -> SoftwareStatus {
    let cache = cached_frpc_path().filter(|p| p.is_file());
    let resolved = resolve_frpc().ok();
    // Prefer showing the cache-managed copy when present.
    let (path, managed, installed) = match (&cache, &resolved) {
        (Some(cache_path), _) => (cache_path.clone(), true, true),
        (None, Some(found)) => (found.clone(), false, true),
        (None, None) => (PathBuf::new(), false, false),
    };
    SoftwareStatus {
        kind: "frpc".into(),
        name: "frp 客户端 (frpc)".into(),
        installed,
        path: path.to_string_lossy().to_string(),
        managed,
    }
}

fn cloudflared_status() -> SoftwareStatus {
    let cache = cached_cloudflared_path().filter(|p| p.is_file());
    let resolved = resolve_cloudflared().ok();
    let (path, managed, installed) = match (&cache, &resolved) {
        (Some(cache_path), _) => (cache_path.clone(), true, true),
        (None, Some(found)) => (found.clone(), false, true),
        (None, None) => (PathBuf::new(), false, false),
    };
    SoftwareStatus {
        kind: "cloudflared".into(),
        name: "Cloudflare Tunnel (cloudflared)".into(),
        installed,
        path: path.to_string_lossy().to_string(),
        managed,
    }
}

/// Report install status for both binaries.
pub fn list_software() -> Vec<SoftwareStatus> {
    vec![frpc_status(), cloudflared_status()]
}

/// Install (download into cache) the requested binary.
pub async fn install_software(kind: &str) -> AppResult<SoftwareStatus> {
    match kind {
        "frpc" => {
            download_frpc_to_cache().await?;
            Ok(frpc_status())
        }
        "cloudflared" => {
            download_cloudflared_to_cache().await?;
            Ok(cloudflared_status())
        }
        other => Err(AppError::Message(format!("未知软件: {other}"))),
    }
}

/// Uninstall a cache-managed binary. Refuses if the binary is not in the cache
/// dir (i.e. it was installed by the system / winget / apt and is not ours).
pub fn uninstall_software(kind: &str) -> AppResult<SoftwareStatus> {
    let cache_path = match kind {
        "frpc" => cached_frpc_path(),
        "cloudflared" => cached_cloudflared_path(),
        other => return Err(AppError::Message(format!("未知软件: {other}"))),
    };

    let Some(path) = cache_path else {
        return Err(AppError::Message("无法解析缓存目录。".into()));
    };

    if path.is_file() {
        std::fs::remove_file(&path)?;
    } else {
        return Err(AppError::Message(
            "该软件不是由本应用安装的，无法在此卸载。".into(),
        ));
    }

    // Also clear any cached download archives for frpc to force a fresh fetch.
    if kind == "frpc" {
        if let Ok(dir) = platform().app_config_dir() {
            let downloads = dir.join("bin").join("downloads");
            let _ = std::fs::remove_dir_all(&downloads);
        }
    }

    Ok(match kind {
        "frpc" => frpc_status(),
        _ => cloudflared_status(),
    })
}
