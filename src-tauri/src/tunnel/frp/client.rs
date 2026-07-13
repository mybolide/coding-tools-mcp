use std::path::{Path, PathBuf};

use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};
use tokio::time::{sleep, Duration};

use crate::error::{AppError, AppResult};
use crate::platform::platform;
use crate::tunnel::cloudflare::stop_child;
use crate::tunnel::supervisor::log_dir_for_profile;
use crate::tunnel::TunnelServiceKind;
use crate::workspace::WorkspaceProfile;

use super::{build_frpc_toml, frp_server_config, FrpServerConfig, VERSION as FRP_VERSION};

const READY_TIMEOUT: Duration = Duration::from_secs(8);

pub struct FrpcHandle {
    pub child: Child,
    pub public_url: String,
    pub pid: Option<u32>,
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

pub async fn spawn_frpc(
    profile: &WorkspaceProfile,
    kind: TunnelServiceKind,
    settings: &crate::settings::AppSettings,
) -> AppResult<FrpcHandle> {
    let frpc = ensure_frpc().await?;
    let config = frp_server_config(profile, kind, settings, None);
    validate_frp_config(&config)?;

    let log_dir = log_dir_for_profile(&profile.id);
    std::fs::create_dir_all(&log_dir)?;
    let config_path = log_dir.join(frpc_config_name(kind));
    let log_path = log_dir.join(frpc_log_name(kind));
    std::fs::write(&config_path, build_frpc_toml(&config))?;

    let mut cmd = Command::new(&frpc);
    cmd.args(["-c", config_path.to_string_lossy().as_ref()]);
    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::piped());
    cmd.current_dir(&profile.path);

    #[cfg(windows)]
    {
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x00000200;
        cmd.creation_flags(CREATE_NEW_PROCESS_GROUP);
    }

    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }

    let use_proxy = match kind {
        TunnelServiceKind::Mcp => profile.tunnel.use_proxy,
        TunnelServiceKind::Actions => profile.actions.use_proxy,
    };
    if use_proxy {
        crate::tunnel::cloudflare::apply_proxy_env(&mut cmd, &settings.proxy);
    }

    let mut child = cmd
        .spawn()
        .map_err(|err| AppError::Message(format!("启动 frpc 失败: {err}")))?;
    let pid = child.id();
    let public_url = public_url_for_config(&config);

    if let Some(stdout) = child.stdout.take() {
        let log_path = log_path.clone();
        tokio::spawn(async move {
            stream_frpc_logs(stdout, &log_path).await;
        });
    }
    if let Some(stderr) = child.stderr.take() {
        let log_path = log_path.clone();
        tokio::spawn(async move {
            stream_frpc_logs(stderr, &log_path).await;
        });
    }

    if !wait_for_frpc_ready(&mut child, &log_path).await? {
        let _ = stop_child(child, pid).await;
        return Err(AppError::Message(
            "frpc 已启动但很快退出。请检查 FRP 服务器地址、端口、Token 与子域名配置。".into(),
        ));
    }

    Ok(FrpcHandle {
        child,
        public_url,
        pid,
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

fn public_url_for_config(config: &FrpServerConfig) -> String {
    format!(
        "https://{}.{}",
        config.proxy.subdomain.trim(),
        config.server_addr.trim().trim_end_matches('/')
    )
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

fn frpc_config_name(kind: TunnelServiceKind) -> &'static str {
    match kind {
        TunnelServiceKind::Mcp => "frpc-mcp.toml",
        TunnelServiceKind::Actions => "frpc-actions.toml",
    }
}

fn frpc_log_name(kind: TunnelServiceKind) -> &'static str {
    match kind {
        TunnelServiceKind::Mcp => "frpc-mcp.log",
        TunnelServiceKind::Actions => "frpc-actions.log",
    }
}

async fn wait_for_frpc_ready(child: &mut Child, log_path: &Path) -> AppResult<bool> {
    let deadline = tokio::time::Instant::now() + READY_TIMEOUT;
    while tokio::time::Instant::now() < deadline {
        if let Some(status) = child.try_wait().map_err(|err| AppError::Message(err.to_string()))? {
            sleep(Duration::from_millis(300)).await;
            return Err(frpc_exit_error(status, log_path));
        }
        if let Some(error) = detect_frpc_log_error(log_path) {
            return Err(error);
        }
        if log_path.is_file() {
            if let Ok(content) = std::fs::read_to_string(log_path) {
                let lowered = strip_ansi(&content).to_ascii_lowercase();
                if lowered.contains("login to server success")
                    || lowered.contains("start proxy success")
                    || lowered.contains("proxy start success")
                {
                    return Ok(true);
                }
            }
        }
        sleep(Duration::from_millis(200)).await;
    }
    Ok(child.try_wait().ok().flatten().is_none())
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

fn detect_frpc_log_error(log_path: &Path) -> Option<AppError> {
    let content = std::fs::read_to_string(log_path).ok()?;
    let lowered = strip_ansi(&content).to_ascii_lowercase();
    if lowered.contains("authorization failed")
        || lowered.contains("token in login doesn't match")
        || lowered.contains("connect to server error")
        || lowered.contains("login to the server failed")
    {
        return Some(AppError::Message(format!(
            "frpc 连接失败：{}",
            frpc_log_summary(log_path)
        )));
    }
    None
}

fn frpc_log_summary(log_path: &Path) -> String {
    let content = std::fs::read_to_string(log_path).unwrap_or_default();
    let cleaned = strip_ansi(&content);
    cleaned
        .lines()
        .map(str::trim)
        .rfind(|line| !line.is_empty())
        .unwrap_or("请检查 FRP 服务器、端口与 Token")
        .to_string()
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

async fn stream_frpc_logs<R>(stderr: R, log_path: &Path)
where
    R: tokio::io::AsyncRead + Unpin + Send + 'static,
{
    if let Some(parent) = log_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let Ok(mut file) = tokio::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path)
        .await
    else {
        return;
    };
    let mut reader = BufReader::new(stderr).lines();
    while let Ok(Some(line)) = reader.next_line().await {
        use tokio::io::AsyncWriteExt;
        let _ = file.write_all(line.as_bytes()).await;
        let _ = file.write_all(b"\n").await;
        let _ = file.flush().await;
    }
}

pub(crate) async fn download_frpc_to_cache() -> AppResult<PathBuf> {
    let settings = crate::settings::AppSettings::load_or_default();
    let (archive_name, binary_in_archive) = frp_release_asset()?;
    let url = format!(
        "https://github.com/fatedier/frp/releases/download/v{FRP_VERSION}/{archive_name}"
    );
    let cache_dir = platform().app_config_dir()?.join("bin").join("downloads");
    std::fs::create_dir_all(&cache_dir)?;
    let archive_path = cache_dir.join(archive_name);
    let dest = cached_frpc_path().expect("cache path");

    if !archive_path.is_file() {
        let bytes = crate::tunnel::download::download_release_asset(&settings, &url, "frpc").await?;
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

fn extract_frpc_from_tar_gz(archive_path: &Path, dest: &Path, binary_suffix: &str) -> AppResult<()> {
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
        Err(AppError::Message(
            "当前平台暂不支持自动下载 frpc。".into(),
        ))
    }
}
