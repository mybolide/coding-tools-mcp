use std::time::{Duration, Instant};

#[cfg(any(target_os = "macos", test))]
use std::path::Path;

use tauri::async_runtime::JoinHandle;

use crate::platform::platform;

pub fn is_own_process(pid: u32) -> bool {
    pid == std::process::id()
}

#[cfg(any(target_os = "macos", test))]
const DESKTOP_EXECUTABLE_NAME: &str = "coding-tools-mcp-desktop";
#[cfg(any(target_os = "macos", test))]
const DESKTOP_BUNDLE_ID: &str = "com.codingtools.mcp.desktop";

/// Reclaim a port only when it belongs to an older macOS instance of this app.
///
/// The normal `is_own_process` check intentionally remains PID-based. This
/// separate path handles a source-built or previously installed app instance
/// that has the same bundle identity but a different PID.
pub fn try_reclaim_previous_macos_app_port(port: u16) -> bool {
    #[cfg(target_os = "macos")]
    {
        let Ok(Some(pid)) = platform().find_pid_listening_on_port(port) else {
            return false;
        };
        if is_own_process(pid) {
            return false;
        }

        let managed_app = platform()
            .process_image_path(pid)
            .ok()
            .flatten()
            .is_some_and(|image| is_managed_macos_desktop_executable(Path::new(&image)));
        if !managed_app {
            return false;
        }

        if let Err(error) = platform().terminate_process_tree(pid) {
            eprintln!("terminate previous macOS app process {pid} failed: {error}");
            return false;
        }

        wait_until_port_is_free_blocking(port, Duration::from_secs(3))
    }

    #[cfg(not(target_os = "macos"))]
    {
        let _ = port;
        false
    }
}

#[cfg(any(target_os = "macos", test))]
fn is_managed_macos_desktop_executable(image: &Path) -> bool {
    if image.file_name().and_then(|name| name.to_str()) != Some(DESKTOP_EXECUTABLE_NAME) {
        return false;
    }

    let Some(bundle) = image
        .ancestors()
        .find(|ancestor| ancestor.extension().and_then(|ext| ext.to_str()) == Some("app"))
    else {
        return false;
    };
    if bundle.file_name().and_then(|name| name.to_str()) != Some("Coding Tools MCP.app")
        || !image.starts_with(bundle.join("Contents").join("MacOS"))
    {
        return false;
    }

    let Ok(info_plist) = std::fs::read_to_string(bundle.join("Contents").join("Info.plist")) else {
        return false;
    };
    let pattern = format!(
        r"(?s)<key>\s*CFBundleIdentifier\s*</key>\s*<string>\s*{}\s*</string>",
        regex::escape(DESKTOP_BUNDLE_ID)
    );
    regex::Regex::new(&pattern)
        .map(|regex| regex.is_match(&info_plist))
        .unwrap_or(false)
}

#[cfg(target_os = "macos")]
fn wait_until_port_is_free_blocking(port: u16, timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        match platform().find_pid_listening_on_port(port) {
            Ok(None) => return true,
            Ok(Some(_)) => std::thread::sleep(Duration::from_millis(50)),
            Err(_) => return false,
        }
    }
    platform()
        .find_pid_listening_on_port(port)
        .ok()
        .flatten()
        .is_none()
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
        // begin_stop 已经发送了优雅退出信号。这里必须等待监听端口真正释放，
        // 不能只把等待任务丢到异步运行时后立即返回，否则 restart 会与旧监听器并发启动。
        let port_free = wait_for_port_free_blocking(port, Duration::from_secs(3));
        if !port_free {
            handle.abort();
        }
        tauri::async_runtime::spawn(async move {
            let _ = handle.await;
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

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    fn write_bundle(root: &std::path::Path, identifier: &str) -> std::path::PathBuf {
        let bundle = root.join("Coding Tools MCP.app");
        let contents = bundle.join("Contents");
        let executable = contents.join("MacOS/coding-tools-mcp-desktop");
        fs::create_dir_all(executable.parent().expect("MacOS dir")).expect("create bundle");
        fs::write(
            contents.join("Info.plist"),
            format!(
                "<?xml version=\"1.0\"?><plist><dict><key>CFBundleIdentifier</key><string>{identifier}</string></dict></plist>"
            ),
        )
        .expect("write Info.plist");
        fs::write(&executable, "test executable").expect("write executable");
        executable
    }

    #[test]
    fn recognizes_a_previous_coding_tools_macos_bundle() {
        let temp = tempfile::tempdir().expect("tempdir");
        let executable = write_bundle(temp.path(), "com.codingtools.mcp.desktop");

        assert!(is_managed_macos_desktop_executable(&executable));
    }

    #[test]
    fn rejects_a_different_macos_bundle_identifier() {
        let temp = tempfile::tempdir().expect("tempdir");
        let executable = write_bundle(temp.path(), "com.example.other-app");

        assert!(!is_managed_macos_desktop_executable(&executable));
    }

    #[test]
    fn rejects_a_matching_identifier_from_a_different_app_bundle() {
        let temp = tempfile::tempdir().expect("tempdir");
        let executable = write_bundle(temp.path(), "com.codingtools.mcp.desktop");
        let other = temp
            .path()
            .join("Other.app/Contents/MacOS/coding-tools-mcp-desktop");
        fs::create_dir_all(other.parent().expect("MacOS dir")).expect("create other bundle");
        fs::copy(
            executable
                .parent()
                .expect("source MacOS dir")
                .parent()
                .expect("source Contents dir")
                .join("Info.plist"),
            other
                .parent()
                .expect("other MacOS dir")
                .parent()
                .expect("other Contents dir")
                .join("Info.plist"),
        )
        .expect("copy Info.plist");
        fs::write(&other, "test executable").expect("write other executable");

        assert!(!is_managed_macos_desktop_executable(&other));
    }
}
