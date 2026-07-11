use std::path::PathBuf;

use crate::platform::paths::{append_if_exists, resolve_from_path};

pub fn cloudflared_candidates() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Some(found) = resolve_from_path("cloudflared") {
        paths.push(found);
    }
    append_if_exists(&mut paths, "/opt/homebrew/bin/cloudflared");
    append_if_exists(&mut paths, "/usr/local/bin/cloudflared");
    if let Some(home) = dirs::home_dir() {
        append_if_exists(&mut paths, home.join(".cloudflared").join("cloudflared"));
    }
    paths
}

pub fn frpc_candidates() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Some(found) = resolve_from_path("frpc") {
        paths.push(found);
    }
    append_if_exists(&mut paths, "/opt/homebrew/bin/frpc");
    append_if_exists(&mut paths, "/usr/local/bin/frpc");
    if let Some(home) = dirs::home_dir() {
        append_if_exists(&mut paths, home.join(".frp").join("frpc"));
    }
    paths
}
