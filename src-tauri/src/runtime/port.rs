use std::time::{Duration, Instant};

use tauri::async_runtime::JoinHandle;

use crate::platform::platform;

pub fn is_own_process(pid: u32) -> bool {
    pid == std::process::id()
}

pub fn try_reclaim_own_port(port: u16) -> bool {
    let Ok(Some(pid)) = platform().find_pid_listening_on_port(port) else {
        return false;
    };
    if !is_own_process(pid) {
        return false;
    }

    match platform().reclaim_listening_port(port) {
        Ok(true) => platform()
            .find_pid_listening_on_port(port)
            .ok()
            .flatten()
            .is_none(),
        Ok(false) => false,
        Err(error) => {
            eprintln!("reclaim_listening_port({port}) failed: {error}");
            false
        }
    }
}

pub async fn wait_for_port_free(port: u16, timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        match platform().find_pid_listening_on_port(port) {
            Ok(None) => return true,
            Ok(Some(pid)) if is_own_process(pid) => {
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
            Ok(Some(_)) => return false,
            Err(_) => return false,
        }
    }

    if try_reclaim_own_port(port) {
        return true;
    }

    platform()
        .find_pid_listening_on_port(port)
        .ok()
        .flatten()
        .is_none()
}

pub fn wait_for_port_free_blocking(port: u16, timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        match platform().find_pid_listening_on_port(port) {
            Ok(None) => return true,
            Ok(Some(pid)) if is_own_process(pid) => {
                std::thread::sleep(Duration::from_millis(50));
            }
            Ok(Some(_)) => return false,
            Err(_) => return false,
        }
    }

    if try_reclaim_own_port(port) {
        return true;
    }

    platform()
        .find_pid_listening_on_port(port)
        .ok()
        .flatten()
        .is_none()
}

pub async fn await_listener_shutdown(handle: Option<JoinHandle<()>>, port: u16) {
    if let Some(handle) = handle {
        let mut handle = handle;
        tokio::select! {
            _ = &mut handle => {}
            _ = tokio::time::sleep(Duration::from_secs(3)) => {
                handle.abort();
                let _ = handle.await;
            }
        }
    }

    if !wait_for_port_free(port, Duration::from_secs(2)).await {
        let _ = try_reclaim_own_port(port);
    }
}

pub fn await_listener_shutdown_blocking(handle: Option<JoinHandle<()>>, port: u16) {
    if let Some(handle) = handle {
        tauri::async_runtime::spawn(async move {
            await_listener_shutdown(Some(handle), port).await;
        });
    } else if !wait_for_port_free_blocking(port, Duration::from_secs(5)) {
        let _ = try_reclaim_own_port(port);
    }
}

pub fn port_busy_message(port: u16, service_label: &str, pid: u32) -> String {
    let image = platform()
        .process_image_path(pid)
        .ok()
        .flatten()
        .unwrap_or_else(|| format!("pid {pid}"));

    if is_own_process(pid) {
        format!(
            "{service_label}端口 {port} 仍被本应用的上一次服务占用（{image}），请先停止服务或稍后再试"
        )
    } else {
        format!("{service_label}端口 {port} 已被占用：{image}")
    }
}
