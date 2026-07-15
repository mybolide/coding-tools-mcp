use std::path::{Path, PathBuf};

use crate::error::{AppError, AppResult};

#[link(name = "proc")]
extern "C" {
    fn proc_pidpath(pid: libc::c_int, buffer: *mut libc::c_void, buffer_size: u32) -> i32;
}

pub fn is_process_alive(pid: u32) -> bool {
    let mut buffer = [0u8; libc::PATH_MAX as usize];
    let size = unsafe { proc_pidpath(pid as i32, buffer.as_mut_ptr().cast(), buffer.len() as u32) };
    size > 0
}

pub fn process_image_path(pid: u32) -> AppResult<Option<String>> {
    let mut buffer = [0u8; libc::PATH_MAX as usize];
    let size = unsafe { proc_pidpath(pid as i32, buffer.as_mut_ptr().cast(), buffer.len() as u32) };
    if size <= 0 {
        return Ok(None);
    }
    let path = std::ffi::CStr::from_bytes_until_nul(&buffer[..size as usize])
        .map_err(|err| AppError::Message(format!("invalid proc path: {err}")))?;
    Ok(Some(path.to_string_lossy().into_owned()))
}

pub fn terminate_process_tree(root_pid: u32) -> AppResult<()> {
    let children = collect_child_pids(root_pid)?;
    for pid in children.iter().copied().rev() {
        signal_pid(pid, libc::SIGTERM)?;
    }
    signal_pid(root_pid, libc::SIGTERM)?;
    std::thread::sleep(std::time::Duration::from_millis(200));
    for pid in &children {
        if is_process_alive(*pid) {
            let _ = signal_pid(*pid, libc::SIGKILL);
        }
    }
    if is_process_alive(root_pid) {
        signal_pid(root_pid, libc::SIGKILL)?;
    }
    Ok(())
}

pub fn terminate_processes_by_image_path(image_path: &Path) -> AppResult<usize> {
    let expected = normalized_image_path(image_path);
    let current_pid = std::process::id();
    let mut terminated = 0;

    for pid in super::net::all_pids()? {
        let pid = pid as u32;
        if pid == current_pid {
            continue;
        }
        let Some(actual) = process_image_path(pid)? else {
            continue;
        };
        if normalized_image_path(Path::new(&actual)) != expected {
            continue;
        }
        terminate_process_tree(pid)?;
        terminated += 1;
    }

    Ok(terminated)
}

fn normalized_image_path(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

fn collect_child_pids(root_pid: u32) -> AppResult<Vec<u32>> {
    let pids = super::net::all_pids()?;
    let mut pending = vec![root_pid];
    let mut seen = std::collections::HashSet::from([root_pid]);
    let mut ordered = Vec::new();

    for pid in pids {
        let Some(parent) = super::net::parent_pid(pid as u32)? else {
            continue;
        };
        if pending.contains(&parent) && seen.insert(pid as u32) {
            ordered.push(pid as u32);
            pending.push(pid as u32);
        }
    }
    Ok(ordered)
}

fn signal_pid(pid: u32, signal: i32) -> AppResult<()> {
    let result = unsafe { libc::kill(pid as i32, signal) };
    if result == 0 || std::io::Error::last_os_error().kind() == std::io::ErrorKind::NotFound {
        return Ok(());
    }
    Err(AppError::Message(format!(
        "kill({pid}, {signal}) failed: {}",
        std::io::Error::last_os_error()
    )))
}
