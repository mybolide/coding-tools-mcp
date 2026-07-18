use std::fs;
use std::path::Path;

use crate::error::{AppError, AppResult};

pub fn find_pid_listening_on_port(port: u16) -> AppResult<Option<u32>> {
    let port_hex = format!("{:04X}", port);
    let content = fs::read_to_string("/proc/net/tcp")
        .map_err(|err| AppError::Message(format!("read /proc/net/tcp failed: {err}")))?;

    for line in content.lines().skip(1) {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 10 {
            continue;
        }
        let local = parts[1];
        let state = parts[3];
        if state != "0A" {
            continue;
        }
        let Some((_, port_part)) = local.split_once(':') else {
            continue;
        };
        if port_part.eq_ignore_ascii_case(&port_hex) {
            let inode = parts[9];
            if let Some(pid) = pid_for_socket_inode(inode)? {
                return Ok(Some(pid));
            }
        }
    }
    Ok(None)
}

fn pid_for_socket_inode(inode: &str) -> AppResult<Option<u32>> {
    let proc = Path::new("/proc");
    let entries =
        fs::read_dir(proc).map_err(|err| AppError::Message(format!("read /proc failed: {err}")))?;

    for entry in entries.flatten() {
        let file_name = entry.file_name();
        let Some(pid_str) = file_name.to_str() else {
            continue;
        };
        if !pid_str.chars().all(|ch| ch.is_ascii_digit()) {
            continue;
        }
        let Ok(pid) = pid_str.parse::<u32>() else {
            continue;
        };
        let fd_dir = entry.path().join("fd");
        let Ok(fds) = fs::read_dir(fd_dir) else {
            continue;
        };
        for fd in fds.flatten() {
            let Ok(link) = fs::read_link(fd.path()) else {
                continue;
            };
            let link_str = link.to_string_lossy();
            if link_str.starts_with("socket:[") && link_str.ends_with(']') {
                let link_inode = &link_str[8..link_str.len() - 1];
                if link_inode == inode {
                    return Ok(Some(pid));
                }
            }
        }
    }
    Ok(None)
}
