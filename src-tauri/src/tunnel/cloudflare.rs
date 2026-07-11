use std::path::{Path, PathBuf};
use std::time::Duration;

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::oneshot;
use tokio::time;

use crate::error::{AppError, AppResult};
use crate::platform::platform;

const READY_TIMEOUT: Duration = Duration::from_secs(12);

/// Handle to a supervised `cloudflared` child process.
pub struct CloudflareTunnelHandle {
    pub child: Child,
    pub public_url: String,
    pub pid: Option<u32>,
}

pub fn resolve_cloudflared() -> AppResult<PathBuf> {
    platform()
        .cloudflared_candidates()
        .into_iter()
        .find(|path| path.is_file())
        .ok_or_else(|| {
            AppError::Message(
                "未找到 cloudflared。请先安装 Cloudflare Tunnel CLI。\n\
                 Windows 可执行：winget install Cloudflare.cloudflared"
                    .into(),
            )
        })
}

pub fn extract_trycloudflare_url(line: &str) -> Option<String> {
    const PREFIX: &str = "https://";
    const SUFFIX: &str = ".trycloudflare.com";
    let lower = line.to_ascii_lowercase();
    let mut search_from = 0;

    while let Some(rel) = lower[search_from..].find(PREFIX) {
        let start = search_from + rel;
        let Some(suffix_rel) = lower[start..].find(SUFFIX) else {
            break;
        };
        let end = start + suffix_rel + SUFFIX.len();
        let host = &line[start + PREFIX.len()..end - SUFFIX.len()];
        if host
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-')
            && !host.is_empty()
        {
            return Some(line[start..end].trim_end_matches('/').to_string());
        }
        search_from = start + PREFIX.len();
    }
    None
}

/// Spawn `cloudflared tunnel --url http://127.0.0.1:{port}` (quick) or named `tunnel run --token`.
pub async fn spawn_cloudflare_tunnel(
    port: u16,
    cwd: &Path,
    log_path: &Path,
    cloudflare_mode: &str,
    cloudflare_token: &str,
    named_public_url: &str,
) -> AppResult<CloudflareTunnelHandle> {
    let cloudflared = resolve_cloudflared()?;
    let quick = cloudflare_mode != "named";

    if !quick {
        if cloudflare_token.trim().is_empty() {
            return Err(AppError::Message(
                "Cloudflare 命名隧道模式需要填写 Tunnel Token。".into(),
            ));
        }
        if named_public_url.trim().is_empty() {
            return Err(AppError::Message(
                "Cloudflare 命名隧道模式需要填写固定公网地址。".into(),
            ));
        }
    }

    let mut cmd = Command::new(&cloudflared);
    cmd.current_dir(cwd);
    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::piped());

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

    if quick {
        cmd.args([
            "tunnel",
            "--url",
            &format!("http://127.0.0.1:{port}"),
        ]);
    } else {
        cmd.args([
            "tunnel",
            "run",
            "--token",
            cloudflare_token.trim(),
        ]);
    }

    let mut child = cmd
        .spawn()
        .map_err(|err| AppError::Message(format!("启动 cloudflared 失败: {err}")))?;
    let pid = child.id();

    if let Some(parent) = log_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let (ready_tx, ready_rx) = oneshot::channel();
    let log_path = log_path.to_path_buf();
    let named_url = named_public_url.trim_end_matches('/').to_string();

    if let Some(stdout) = child.stdout.take() {
        let stderr = child.stderr.take();
        tokio::spawn(async move {
            stream_cloudflare_output(stdout, stderr, &log_path, quick, named_url, ready_tx).await;
        });
    } else {
        let _ = ready_tx.send(QuickTunnelReady {
            public_url: if quick {
                None
            } else {
                Some(named_public_url.trim_end_matches('/').to_string())
            },
            named_ready: !quick,
        });
    }

    let ready = time::timeout(READY_TIMEOUT, ready_rx)
        .await
        .map_err(|_| {
            AppError::Message(
                "cloudflared 已启动，但在预期时间内没有返回 trycloudflare.com 公网地址。".into(),
            )
        })?
        .map_err(|_| AppError::Message("cloudflared 输出流意外结束。".into()))?;

    let public_url = if quick {
        ready.public_url.ok_or_else(|| {
            AppError::Message(
                "cloudflared 已启动，但在预期时间内没有返回 trycloudflare.com 公网地址。".into(),
            )
        })?
    } else {
        named_public_url.trim_end_matches('/').to_string()
    };

    Ok(CloudflareTunnelHandle {
        child,
        public_url,
        pid,
    })
}

struct QuickTunnelReady {
    public_url: Option<String>,
    #[allow(dead_code)]
    named_ready: bool,
}

async fn stream_cloudflare_output<R, E>(
    stdout: R,
    stderr: Option<E>,
    log_path: &Path,
    quick: bool,
    named_url: String,
    ready_tx: oneshot::Sender<QuickTunnelReady>,
) where
    R: tokio::io::AsyncRead + Unpin + Send + 'static,
    E: tokio::io::AsyncRead + Unpin + Send + 'static,
{
    let mut ready_tx = Some(ready_tx);
    let mut public_url: Option<String> = None;

    let mut log = match tokio::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path)
        .await
    {
        Ok(file) => file,
        Err(_) => {
            if let Some(tx) = ready_tx.take() {
                let _ = tx.send(QuickTunnelReady {
                    public_url: if quick { None } else { Some(named_url) },
                    named_ready: !quick,
                });
            }
            return;
        }
    };

    let mut send_ready = |url: Option<String>, named_ready: bool| {
        if let Some(tx) = ready_tx.take() {
            let _ = tx.send(QuickTunnelReady {
                public_url: url,
                named_ready,
            });
        }
    };

    let mut handle_line = |line: &str| {
        if quick {
            if public_url.is_none() {
                if let Some(url) = extract_trycloudflare_url(line) {
                    public_url = Some(url.clone());
                    send_ready(Some(url), false);
                }
            }
        } else {
            let lowered = line.to_ascii_lowercase();
            if lowered.contains("registered tunnel connection")
                || lowered.contains("starting metrics server")
            {
                send_ready(Some(named_url.clone()), true);
            }
        }
    };

    let mut stdout = BufReader::new(stdout).lines();
    while let Ok(Some(line)) = stdout.next_line().await {
        let _ = log.write_all(line.as_bytes()).await;
        let _ = log.write_all(b"\n").await;
        let _ = log.flush().await;
        handle_line(&line);
    }

    if let Some(stderr) = stderr {
        let mut stderr = BufReader::new(stderr).lines();
        while let Ok(Some(line)) = stderr.next_line().await {
            let _ = log.write_all(line.as_bytes()).await;
            let _ = log.write_all(b"\n").await;
            let _ = log.flush().await;
            handle_line(&line);
        }
    }

    send_ready(public_url, !quick);
}

pub async fn stop_child(mut child: Child, pid: Option<u32>) -> AppResult<()> {
    if let Some(pid) = pid {
        let _ = platform().terminate_process_tree(pid);
    }

    let _ = child.kill().await;
    let _ = time::timeout(Duration::from_secs(3), child.wait()).await;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::extract_trycloudflare_url;

    #[test]
    fn extracts_trycloudflare_url_from_log_line() {
        let line = "INF | https://abc-def.trycloudflare.com is your tunnel URL";
        assert_eq!(
            extract_trycloudflare_url(line).as_deref(),
            Some("https://abc-def.trycloudflare.com")
        );
    }

    #[test]
    fn ignores_invalid_hosts() {
        let line = "https://bad_host.trycloudflare.com";
        assert!(extract_trycloudflare_url(line).is_none());
    }
}
