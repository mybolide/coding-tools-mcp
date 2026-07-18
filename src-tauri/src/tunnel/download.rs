use std::time::Duration;

use crate::error::{AppError, AppResult};
use crate::settings::AppSettings;

/// A GitHub download URL rewritten to honor the configured mirror, plus the
/// original URL as a fallback. Callers try `primary` first, then `fallback`.
pub struct DownloadPlan {
    pub primary: String,
    pub fallback: Option<String>,
}

/// Build the ordered list of URLs to attempt for a GitHub release asset.
///
/// When a mirror prefix is configured we try the mirrored URL first and keep
/// the original GitHub URL as a fallback. With no mirror we just hit GitHub.
pub fn plan_github_download(settings: &AppSettings, github_url: &str) -> DownloadPlan {
    let prefix = settings.download.github_mirror.trim();
    if prefix.is_empty() {
        return DownloadPlan {
            primary: github_url.to_string(),
            fallback: None,
        };
    }
    let mirrored = format!(
        "{}/{}",
        prefix.trim_end_matches('/'),
        github_url.trim_start_matches('/')
    );
    DownloadPlan {
        primary: mirrored,
        fallback: Some(github_url.to_string()),
    }
}

/// Build a reqwest client honoring the configured proxy mode.
///
/// - `none`: no proxy (default direct connection)
/// - `system`: reqwest's built-in system-proxy detection
/// - anything else: treated as an explicit proxy URL (http/https/socks5)
fn build_client(settings: &AppSettings) -> AppResult<reqwest::Client> {
    let mut builder = reqwest::Client::builder().timeout(Duration::from_secs(180));
    let mode = settings.download.proxy_mode.trim();
    match mode {
        "" | "none" => {
            builder = builder.no_proxy();
        }
        "system" => {
            // Leave reqwest's default system-proxy detection enabled.
        }
        url => {
            let proxy = reqwest::Proxy::all(url)
                .map_err(|err| AppError::Message(format!("代理地址无效: {err}")))?;
            builder = builder.proxy(proxy);
        }
    }
    builder
        .build()
        .map_err(|err| AppError::Message(err.to_string()))
}

/// Download bytes from a GitHub release asset, honoring mirror + proxy config.
///
/// Tries the mirrored URL first (if configured), then falls back to the
/// original GitHub URL. `label` is used in error messages (e.g. "frpc").
pub async fn download_release_asset(
    settings: &AppSettings,
    github_url: &str,
    label: &str,
) -> AppResult<Vec<u8>> {
    let plan = plan_github_download(settings, github_url);
    let client = build_client(settings)?;

    let mut urls = vec![plan.primary];
    if let Some(fallback) = plan.fallback {
        urls.push(fallback);
    }

    let mut last_err = String::new();
    for url in urls {
        match fetch_bytes(&client, &url).await {
            Ok(bytes) => return Ok(bytes),
            Err(err) => {
                last_err = err;
            }
        }
    }
    Err(AppError::Message(format!("下载 {label} 失败: {last_err}")))
}

async fn fetch_bytes(client: &reqwest::Client, url: &str) -> Result<Vec<u8>, String> {
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|err| err.to_string())?
        .error_for_status()
        .map_err(|err| err.to_string())?;
    let bytes = response.bytes().await.map_err(|err| err.to_string())?;
    Ok(bytes.to_vec())
}
