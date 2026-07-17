use std::collections::HashSet;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::time::Instant;

use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};
use tokio::time::{sleep, Duration};

use crate::error::{AppError, AppResult};
use crate::platform::platform;
use crate::tunnel::cloudflare::stop_child;
use crate::tunnel::supervisor::log_dir_for_profile;
use crate::tunnel::TunnelServiceKind;
use crate::workspace::WorkspaceProfile;

use super::{
    build_frpc_toml_for_routes, frp_server_config, FrpServerConfig, VERSION as FRP_VERSION,
};

const READY_TIMEOUT: Duration = Duration::from_secs(8);
const FRPC_RESTART_GRACE: Duration = Duration::from_millis(600);
const FRPC_OPERATION_LOCK_TIMEOUT: Duration = Duration::from_secs(15);
const FRPC_STALE_LOCK_AFTER: Duration = Duration::from_secs(30);

pub struct FrpcHandle {
    pub child: Child,
    pub pid: Option<u32>,
}

fn managed_frpc_dir(workspace_id: &str) -> AppResult<PathBuf> {
    let dir = platform()
        .app_config_dir()?
        .join("frpc")
        .join(sanitize_workspace_id(workspace_id));
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

pub(crate) fn managed_frpc_config_path(workspace_id: &str) -> AppResult<PathBuf> {
    Ok(managed_frpc_dir(workspace_id)?.join("frpc.toml"))
}

pub(crate) fn managed_frpc_pid_path(workspace_id: &str) -> AppResult<PathBuf> {
    Ok(managed_frpc_dir(workspace_id)?.join("frpc.pid"))
}

fn managed_frpc_operation_lock_path(workspace_id: &str) -> AppResult<PathBuf> {
    Ok(managed_frpc_dir(workspace_id)?.join("frpc-operation.lock"))
}

fn sanitize_workspace_id(workspace_id: &str) -> String {
    let sanitized: String = workspace_id
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .collect();
    if sanitized.is_empty() {
        "workspace".into()
    } else {
        sanitized
    }
}

pub(crate) fn managed_frpc_config_matches(workspace_id: &str, expected: &str) -> AppResult<bool> {
    let path = managed_frpc_config_path(workspace_id)?;
    let actual = match std::fs::read_to_string(path) {
        Ok(content) => content,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => {
            return Err(AppError::Message(format!(
                "读取共享 frpc 配置失败：{error}"
            )))
        }
    };
    Ok(actual == expected)
}

/// 跨应用实例串行化 frpc 的停止与启动。
///
/// 同一个进程内由 `TunnelSupervisor` 的 Tokio mutex 保证串行；但用户
/// 可能在旧实例尚未完全退出时启动新实例，此时仅靠内存 mutex 仍会出现
/// stop/spawn 交叉，最终留下两个 frpc。这个短生命周期锁把整个替换过程
/// 扩展到应用进程之间，并在持有者崩溃后允许新实例回收过期锁。
pub(crate) struct FrpcOperationLock {
    path: PathBuf,
    _file: std::fs::File,
}

impl Drop for FrpcOperationLock {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

pub(crate) async fn acquire_frpc_operation_lock(
    workspace_id: &str,
) -> AppResult<FrpcOperationLock> {
    let path = managed_frpc_operation_lock_path(workspace_id)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let started = Instant::now();
    loop {
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
        {
            Ok(mut file) => {
                writeln!(file, "{}", std::process::id())?;
                return Ok(FrpcOperationLock { path, _file: file });
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                if stale_lock(&path) {
                    let _ = std::fs::remove_file(&path);
                    continue;
                }
                if started.elapsed() >= FRPC_OPERATION_LOCK_TIMEOUT {
                    return Err(AppError::Message(
                        "等待另一个 frpc 操作完成超时，请稍后重试。".into(),
                    ));
                }
                sleep(Duration::from_millis(50)).await;
            }
            Err(error) => {
                return Err(AppError::Message(format!("创建 frpc 操作锁失败：{error}")));
            }
        }
    }
}

fn stale_lock(path: &Path) -> bool {
    if let Ok(contents) = std::fs::read_to_string(path) {
        if let Some(pid) = contents
            .lines()
            .next()
            .and_then(|line| line.trim().parse().ok())
        {
            return !platform().is_process_alive(pid);
        }
    }

    std::fs::metadata(path)
        .and_then(|metadata| metadata.modified())
        .ok()
        .and_then(|modified| modified.elapsed().ok())
        .is_some_and(|age| age >= FRPC_STALE_LOCK_AFTER)
}

pub fn resolve_frpc() -> AppResult<PathBuf> {
    bundled_frpc()
        .or_else(|| {
            platform()
                .frpc_candidates()
                .into_iter()
                .find(|path| path.is_file())
        })
        .or_else(|| cached_frpc_path().filter(|path| path.is_file()))
        .ok_or_else(|| {
            AppError::Message(
                "未找到 frpc。连接隧道时将尝试自动下载；也可自行安装 frp 客户端。".into(),
            )
        })
}

pub async fn ensure_frpc() -> AppResult<PathBuf> {
    if let Ok(path) = resolve_frpc() {
        return Ok(path);
    }
    download_frpc_to_cache().await
}

fn write_managed_frpc_pid(workspace_id: &str, pid: u32, image_path: &Path) -> AppResult<()> {
    let path = managed_frpc_pid_path(workspace_id)?;
    std::fs::write(path, format!("{pid}\n{}\n", image_path.to_string_lossy()))?;
    Ok(())
}

pub(crate) fn clear_managed_frpc_pid(workspace_id: &str) {
    if let Ok(path) = managed_frpc_pid_path(workspace_id) {
        let _ = std::fs::remove_file(path);
    }
}

pub(crate) async fn stop_recorded_frpc_instance(workspace_id: &str) -> AppResult<bool> {
    let path = managed_frpc_pid_path(workspace_id)?;
    let contents = match std::fs::read_to_string(&path) {
        Ok(contents) => contents,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error.into()),
    };
    let mut lines = contents.lines();
    let pid = lines
        .next()
        .and_then(|value| value.trim().parse::<u32>().ok());
    let recorded_image = lines
        .next()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let (Some(pid), Some(recorded_image)) = (pid, recorded_image) else {
        clear_managed_frpc_pid(workspace_id);
        return Ok(false);
    };

    if !platform().is_process_alive(pid) {
        clear_managed_frpc_pid(workspace_id);
        return Ok(false);
    }
    let Some(actual_image) = platform().process_image_path(pid)? else {
        clear_managed_frpc_pid(workspace_id);
        return Ok(false);
    };
    if !same_process_image(Path::new(recorded_image), Path::new(&actual_image)) {
        clear_managed_frpc_pid(workspace_id);
        return Ok(false);
    }

    platform().terminate_process_tree(pid)?;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(3);
    while platform().is_process_alive(pid) && tokio::time::Instant::now() < deadline {
        sleep(Duration::from_millis(50)).await;
    }
    if platform().is_process_alive(pid) {
        return Err(AppError::Message(format!(
            "停止工作区 frpc 超时，PID {pid} 仍在运行。"
        )));
    }
    clear_managed_frpc_pid(workspace_id);
    sleep(FRPC_RESTART_GRACE).await;
    Ok(true)
}

fn same_process_image(left: &Path, right: &Path) -> bool {
    let left = std::fs::canonicalize(left).unwrap_or_else(|_| left.to_path_buf());
    let right = std::fs::canonicalize(right).unwrap_or_else(|_| right.to_path_buf());
    #[cfg(windows)]
    {
        left.to_string_lossy()
            .trim_start_matches("\\\\?\\")
            .eq_ignore_ascii_case(right.to_string_lossy().trim_start_matches("\\\\?\\"))
    }
    #[cfg(not(windows))]
    {
        left == right
    }
}

pub async fn spawn_frpc(
    workspace_id: &str,
    routes: &[(&WorkspaceProfile, TunnelServiceKind)],
    settings: &crate::settings::AppSettings,
) -> AppResult<FrpcHandle> {
    let Some((first_profile, _)) = routes.first() else {
        return Err(AppError::Message("没有可启动的 FRP 线路。".into()));
    };
    let frpc = ensure_frpc().await?;

    let configs: Vec<FrpServerConfig> = routes
        .iter()
        .map(|(profile, kind)| frp_server_config(profile, *kind, settings, None))
        .collect();
    for config in &configs {
        validate_frp_config(config)?;
    }

    let config_path = managed_frpc_config_path(workspace_id)?;
    let log_paths: Vec<PathBuf> = routes
        .iter()
        .map(|(profile, kind)| -> AppResult<PathBuf> {
            let profile_log_dir = log_dir_for_profile(&profile.id);
            std::fs::create_dir_all(&profile_log_dir)?;
            Ok(profile_log_dir.join(frpc_log_name(*kind)))
        })
        .collect::<AppResult<Vec<_>>>()?;
    let log_path = log_paths
        .first()
        .cloned()
        .ok_or_else(|| AppError::Message("没有可写入的 frpc 日志路径。".into()))?;
    let config_text = build_frpc_toml_for_routes(&configs);
    std::fs::write(&config_path, &config_text)?;
    let log_offset = log_file_len(&log_path);

    let mut cmd = Command::new(&frpc);
    cmd.args(["-c", config_path.to_string_lossy().as_ref()]);
    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::piped());
    cmd.current_dir(&first_profile.path);

    #[cfg(windows)]
    {
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x00000200;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        // frpc 是控制台程序。显式禁止创建控制台，避免安装版或开发版
        // 从桌面应用启动时短暂闪出黑色窗口。
        cmd.creation_flags(CREATE_NEW_PROCESS_GROUP | CREATE_NO_WINDOW);
    }

    #[cfg(unix)]
    {
        cmd.process_group(0);
    }

    // 一个聚合 frpc 只有一套进程环境，不能按工作区分别设置代理。
    // 任一路由要求使用代理时，为整个聚合连接启用代理；这样 HashMap
    // 的迭代顺序不会随机决定最终行为，也不会因偏好不同丢弃其它路由。
    let use_proxy = aggregate_uses_proxy(routes);
    if use_proxy {
        crate::tunnel::cloudflare::apply_proxy_env(&mut cmd, &settings.proxy);
    }

    let mut child = cmd
        .spawn()
        .map_err(|err| AppError::Message(format!("启动 frpc 失败: {err}")))?;
    let pid = child.id();
    if let Some(pid) = pid {
        if let Err(error) = write_managed_frpc_pid(workspace_id, pid, &frpc) {
            let _ = stop_child(child, Some(pid)).await;
            return Err(error);
        }
    }
    if let Some(stdout) = child.stdout.take() {
        let log_paths = log_paths.clone();
        tokio::spawn(async move {
            stream_frpc_logs(stdout, log_paths).await;
        });
    }
    if let Some(stderr) = child.stderr.take() {
        let log_paths = log_paths.clone();
        tokio::spawn(async move {
            stream_frpc_logs(stderr, log_paths).await;
        });
    }

    let ready = match wait_for_frpc_ready(&mut child, &log_path, log_offset, configs.len()).await {
        Ok(ready) => ready,
        Err(error) => {
            let _ = stop_child(child, pid).await;
            clear_managed_frpc_pid(workspace_id);
            return Err(error);
        }
    };
    if !ready {
        let _ = stop_child(child, pid).await;
        clear_managed_frpc_pid(workspace_id);
        return Err(AppError::Message(
            "frpc 已启动但很快退出。请检查 FRP 服务器地址、端口、Token 与子域名配置。".into(),
        ));
    }

    Ok(FrpcHandle { child, pid })
}

fn aggregate_uses_proxy(routes: &[(&WorkspaceProfile, TunnelServiceKind)]) -> bool {
    routes.iter().any(|(profile, kind)| match kind {
        TunnelServiceKind::Mcp => profile.tunnel.use_proxy,
        TunnelServiceKind::Actions => profile.actions.use_proxy,
    })
}

#[allow(dead_code)]
pub async fn stop_frpc(child: Child, pid: Option<u32>) -> AppResult<()> {
    stop_child(child, pid).await
}

fn validate_frp_config(config: &FrpServerConfig) -> AppResult<()> {
    if config.server_addr.trim().is_empty() {
        return Err(AppError::Message("FRP 模式需要填写服务器域名。".into()));
    }
    if config.proxy.subdomain.trim().is_empty() {
        return Err(AppError::Message("FRP 模式需要填写子域名。".into()));
    }
    if config.server_port == 0 {
        return Err(AppError::Message("FRP 服务器端口无效。".into()));
    }
    Ok(())
}

fn bundled_frpc() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let dir = exe.parent()?;
    #[cfg(windows)]
    let names = ["frpc.exe"];
    #[cfg(not(windows))]
    let names = ["frpc"];
    for name in names {
        let candidate = dir.join(name);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

pub(crate) fn cached_frpc_path() -> Option<PathBuf> {
    platform()
        .app_config_dir()
        .ok()
        .map(|dir| dir.join("bin").join(frpc_binary_name()))
}

pub(crate) fn frpc_binary_name() -> &'static str {
    #[cfg(windows)]
    {
        "frpc.exe"
    }
    #[cfg(not(windows))]
    {
        "frpc"
    }
}

fn frpc_log_name(kind: TunnelServiceKind) -> &'static str {
    match kind {
        TunnelServiceKind::Mcp => "frpc-mcp.log",
        TunnelServiceKind::Actions => "frpc-actions.log",
    }
}

async fn wait_for_frpc_ready(
    child: &mut Child,
    log_path: &Path,
    log_offset: u64,
    expected_proxy_count: usize,
) -> AppResult<bool> {
    let deadline = tokio::time::Instant::now() + READY_TIMEOUT;
    while tokio::time::Instant::now() < deadline {
        if let Some(status) = child
            .try_wait()
            .map_err(|err| AppError::Message(err.to_string()))?
        {
            sleep(Duration::from_millis(300)).await;
            return Err(frpc_exit_error(status, log_path));
        }
        if let Some(error) = detect_frpc_log_error(log_path, log_offset) {
            return Err(error);
        }
        let content = read_log_since(log_path, log_offset);
        if successful_proxy_names(&content).len() >= expected_proxy_count {
            return Ok(true);
        }
        sleep(Duration::from_millis(200)).await;
    }
    if child.try_wait().ok().flatten().is_none() {
        let detail = read_log_since(log_path, log_offset);
        let detail = if detail.trim().is_empty() {
            "尚未收到 frpc 登录或代理建立日志".to_string()
        } else {
            frpc_log_summary_from_text(&detail)
        };
        return Err(AppError::Message(format!(
            "frpc 启动超时，隧道尚未就绪：{detail}"
        )));
    }
    Ok(false)
}

fn frpc_exit_error(status: std::process::ExitStatus, log_path: &Path) -> AppError {
    let detail = frpc_log_summary(log_path);
    if detail.is_empty() {
        return AppError::Message(format!(
            "frpc 退出，状态码 {status}。请检查 FRP 服务器地址、端口、Token 与子域名；\
             若使用全局 FRP 配置，请在工作区隧道里选择对应配置。"
        ));
    }
    AppError::Message(format!("frpc 退出，状态码 {status}。{detail}"))
}

fn detect_frpc_log_error(log_path: &Path, log_offset: u64) -> Option<AppError> {
    let content = read_log_since(log_path, log_offset);
    if content.is_empty() {
        return None;
    }
    let lowered = strip_ansi(&content).to_ascii_lowercase();
    if lowered.contains("authorization failed")
        || lowered.contains("token in login doesn't match")
        || lowered.contains("login to the server failed")
        || lowered.contains("start error: proxy")
        || lowered.contains("proxy already exists")
    {
        return Some(AppError::Message(format!(
            "frpc 连接失败：{}",
            frpc_log_summary_from_text(&content)
        )));
    }
    None
}

fn frpc_log_summary(log_path: &Path) -> String {
    let content = std::fs::read_to_string(log_path).unwrap_or_default();
    frpc_log_summary_from_text(&content)
}

fn frpc_log_summary_from_text(content: &str) -> String {
    let cleaned = strip_ansi(content);
    cleaned
        .lines()
        .map(str::trim)
        .rfind(|line| !line.is_empty())
        .unwrap_or("请检查 FRP 服务器、端口与 Token")
        .to_string()
}

fn log_file_len(path: &Path) -> u64 {
    std::fs::metadata(path)
        .map(|metadata| metadata.len())
        .unwrap_or(0)
}

fn read_log_since(path: &Path, offset: u64) -> String {
    let Ok(mut file) = std::fs::File::open(path) else {
        return String::new();
    };
    let current_len = file.metadata().map(|metadata| metadata.len()).unwrap_or(0);
    let start = if current_len >= offset { offset } else { 0 };
    if file.seek(SeekFrom::Start(start)).is_err() {
        return String::new();
    }
    let mut bytes = Vec::new();
    if file.read_to_end(&mut bytes).is_err() {
        return String::new();
    }
    String::from_utf8_lossy(&bytes).into_owned()
}

fn strip_ansi(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\u{1b}' {
            if chars.peek() == Some(&'[') {
                chars.next();
                for c in chars.by_ref() {
                    if c.is_ascii_alphabetic() {
                        break;
                    }
                }
            }
            continue;
        }
        out.push(ch);
    }
    out
}

async fn stream_frpc_logs<R>(stderr: R, log_paths: Vec<PathBuf>)
where
    R: tokio::io::AsyncRead + Unpin + Send + 'static,
{
    let mut files = Vec::new();
    for log_path in log_paths {
        if let Some(parent) = log_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(file) = tokio::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(log_path)
            .await
        {
            files.push(file);
        }
    }
    if files.is_empty() {
        return;
    }

    let mut reader = BufReader::new(stderr).lines();
    while let Ok(Some(line)) = reader.next_line().await {
        use tokio::io::AsyncWriteExt;
        for file in &mut files {
            let _ = file.write_all(line.as_bytes()).await;
            let _ = file.write_all(b"\n").await;
            let _ = file.flush().await;
        }
    }
}

fn successful_proxy_names(content: &str) -> HashSet<String> {
    let cleaned = strip_ansi(content);
    cleaned
        .lines()
        .filter_map(|line| {
            let lower = line.to_ascii_lowercase();
            let marker = if lower.contains("start proxy success") {
                "start proxy success"
            } else if lower.contains("proxy start success") {
                "proxy start success"
            } else {
                return None;
            };

            let marker_index = lower.find(marker)?;
            line[..marker_index]
                .rsplit_once('[')
                .map(|(_, name)| name.trim().trim_end_matches(']').trim().to_string())
                .filter(|name| !name.is_empty())
        })
        .collect()
}

pub(crate) async fn download_frpc_to_cache() -> AppResult<PathBuf> {
    let settings = crate::settings::AppSettings::load_or_default();
    let (archive_name, binary_in_archive) = frp_release_asset()?;
    let url =
        format!("https://github.com/fatedier/frp/releases/download/v{FRP_VERSION}/{archive_name}");
    let cache_dir = platform().app_config_dir()?.join("bin").join("downloads");
    std::fs::create_dir_all(&cache_dir)?;
    let archive_path = cache_dir.join(archive_name);
    let dest = cached_frpc_path().expect("cache path");

    if !archive_path.is_file() {
        let bytes =
            crate::tunnel::download::download_release_asset(&settings, &url, "frpc").await?;
        std::fs::write(&archive_path, bytes)?;
    }

    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }

    if archive_name.ends_with(".zip") {
        extract_frpc_from_zip(&archive_path, &dest, binary_in_archive)?;
    } else {
        extract_frpc_from_tar_gz(&archive_path, &dest, binary_in_archive)?;
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(meta) = std::fs::metadata(&dest) {
            let mut perms = meta.permissions();
            perms.set_mode(0o755);
            let _ = std::fs::set_permissions(&dest, perms);
        }
    }

    if dest.is_file() {
        Ok(dest)
    } else {
        Err(AppError::Message("frpc 自动安装失败。".into()))
    }
}

fn extract_frpc_from_zip(archive_path: &Path, dest: &Path, binary_suffix: &str) -> AppResult<()> {
    let file = std::fs::File::open(archive_path)?;
    let mut archive = zip::ZipArchive::new(file)
        .map_err(|err| AppError::Message(format!("解压 frpc 安装包失败: {err}")))?;
    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .map_err(|err| AppError::Message(format!("读取 frpc 安装包失败: {err}")))?;
        let name = entry.name().replace('\\', "/");
        if name.ends_with(binary_suffix) || name.ends_with("frpc") || name.ends_with("frpc.exe") {
            let mut out = std::fs::File::create(dest)?;
            std::io::copy(&mut entry, &mut out)?;
            return Ok(());
        }
    }
    Err(AppError::Message(
        "frpc 安装包中未找到 frpc 可执行文件。".into(),
    ))
}

fn extract_frpc_from_tar_gz(
    archive_path: &Path,
    dest: &Path,
    binary_suffix: &str,
) -> AppResult<()> {
    let file = std::fs::File::open(archive_path)?;
    let decoder = flate2::read::GzDecoder::new(file);
    let mut archive = tar::Archive::new(decoder);
    for entry in archive
        .entries()
        .map_err(|err| AppError::Message(format!("解压 frpc 安装包失败: {err}")))?
    {
        let mut entry =
            entry.map_err(|err| AppError::Message(format!("读取 frpc 安装包失败: {err}")))?;
        let path = entry
            .path()
            .map_err(|err| AppError::Message(err.to_string()))?
            .to_string_lossy()
            .replace('\\', "/");
        if path.ends_with(binary_suffix) || path.ends_with("/frpc") {
            let mut out = std::fs::File::create(dest)?;
            std::io::copy(&mut entry, &mut out)?;
            return Ok(());
        }
    }
    Err(AppError::Message(
        "frpc 安装包中未找到 frpc 可执行文件。".into(),
    ))
}

fn frp_release_asset() -> AppResult<(&'static str, &'static str)> {
    #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
    {
        Ok(("frp_0.61.2_windows_amd64.zip", "frpc.exe"))
    }
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    {
        Ok(("frp_0.61.2_linux_amd64.tar.gz", "frpc"))
    }
    #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
    {
        Ok(("frp_0.61.2_linux_arm64.tar.gz", "frpc"))
    }
    #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
    {
        Ok(("frp_0.61.2_darwin_amd64.tar.gz", "frpc"))
    }
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    {
        Ok(("frp_0.61.2_darwin_arm64.tar.gz", "frpc"))
    }
    #[cfg(not(any(
        all(target_os = "windows", target_arch = "x86_64"),
        all(target_os = "linux", target_arch = "x86_64"),
        all(target_os = "linux", target_arch = "aarch64"),
        all(target_os = "macos", target_arch = "x86_64"),
        all(target_os = "macos", target_arch = "aarch64"),
    )))]
    {
        Err(AppError::Message("当前平台暂不支持自动下载 frpc。".into()))
    }
}

#[cfg(test)]
mod tests {
    use super::{
        aggregate_uses_proxy, managed_frpc_config_path, managed_frpc_pid_path,
        successful_proxy_names,
    };
    use crate::tunnel::TunnelServiceKind;
    use crate::workspace::WorkspaceProfile;

    #[test]
    fn login_success_alone_is_not_a_ready_proxy() {
        let log = "login to server success, get run id [run-id]";
        assert!(successful_proxy_names(log).is_empty());
    }

    #[test]
    fn all_distinct_proxy_successes_are_counted() {
        let log = concat!(
            "[run-id] [first-mcp] start proxy success\n",
            "[run-id] [first-mcp] start proxy success\n",
            "[run-id] [second-mcp] proxy start success\n",
        );
        let names = successful_proxy_names(log);
        assert_eq!(names.len(), 2);
        assert!(names.contains("first-mcp"));
        assert!(names.contains("second-mcp"));
    }

    #[test]
    fn aggregate_proxy_is_enabled_when_any_route_requests_it() {
        let mut direct = WorkspaceProfile::new("C:/workspace/direct".into(), None);
        direct.tunnel.use_proxy = false;
        let mut proxied = WorkspaceProfile::new("C:/workspace/proxied".into(), None);
        proxied.tunnel.use_proxy = true;

        assert!(aggregate_uses_proxy(&[
            (&direct, TunnelServiceKind::Mcp),
            (&proxied, TunnelServiceKind::Mcp),
        ]));
        assert!(aggregate_uses_proxy(&[
            (&proxied, TunnelServiceKind::Mcp),
            (&direct, TunnelServiceKind::Mcp),
        ]));
        assert!(!aggregate_uses_proxy(&[(&direct, TunnelServiceKind::Mcp)]));
    }

    #[test]
    fn managed_config_paths_are_isolated_by_workspace() {
        let first = managed_frpc_config_path("first-workspace").unwrap();
        let second = managed_frpc_config_path("second-workspace").unwrap();

        assert_ne!(first, second);
        assert!(first.ends_with("first-workspace/frpc.toml"));
        assert!(second.ends_with("second-workspace/frpc.toml"));
    }

    #[test]
    fn managed_pid_paths_are_isolated_and_sanitize_workspace_ids() {
        let path = managed_frpc_pid_path("../unsafe workspace").unwrap();
        let normalized = path.to_string_lossy().replace('\\', "/");

        assert!(normalized.ends_with("frpc/___unsafe_workspace/frpc.pid"));
        assert!(!normalized.contains("/../"));
    }
}
