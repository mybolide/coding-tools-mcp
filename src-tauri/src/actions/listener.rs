use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use axum::{
    extract::{Form, Path, Query, State},
    http::{HeaderMap, StatusCode},
    middleware,
    response::{Html, IntoResponse, Json, Response},
    routing::{get, post},
    Extension, Router,
};
use serde_json::{json, Value};
use tokio::sync::{oneshot, Mutex, RwLock};
use tower_http::cors::CorsLayer;

use crate::auth::{
    authorization_server_metadata, authorize_get, authorize_post, oauth_base_url,
    token_exchange, AuthorizeForm, AuthorizeParams, OAuthRuntime, TokenForm,
};
use crate::tools::{self, is_allowed_tool, policy::PolicySettings, wrap_tool_result, ToolContext};
use crate::tunnel::append_profile_log;

use super::auth::{require_actions_auth, AuthConfig};
use super::openapi;

pub type ShutdownSender = oneshot::Sender<()>;

#[derive(Clone)]
struct AppState {
    ctx: Arc<ToolContext>,
    openapi: Arc<RwLock<Value>>,
    auth: Arc<AuthConfig>,
    workspace_path: String,
    oauth: Option<Arc<OAuthRuntime>>,
    oauth_base_url: String,
    oauth_client_secret: Option<String>,
    write_lock: Arc<Mutex<()>>,
}

pub fn spawn_listener(
    workspace_id: &str,
    actions_port: u16,
    workspace_path: PathBuf,
    public_base_url: String,
    auth_type: String,
    api_key: Option<String>,
    oauth_client_id: String,
    oauth_client_secret: Option<String>,
    oauth_password: Option<String>,
    oauth_token_secret: Option<String>,
    policy: PolicySettings,
) -> Result<(ShutdownSender, tauri::async_runtime::JoinHandle<()>), String> {
    if auth_type == "api_key" && api_key.as_ref().is_none_or(String::is_empty) {
        return Err("Actions API key is not configured".into());
    }
    if auth_type == "oauth" {
        if oauth_password.as_ref().is_none_or(String::is_empty) {
            return Err("Actions OAuth password is not configured".into());
        }
        if oauth_token_secret.as_ref().is_none_or(String::is_empty) {
            return Err("Actions OAuth token secret is not configured".into());
        }
    }

    let oauth_base = oauth_base_url(&public_base_url, actions_port);
    let oauth = if auth_type == "oauth" {
        Some(Arc::new(OAuthRuntime::new(
            oauth_base.clone(),
            oauth_client_id,
            oauth_client_secret.clone(),
            oauth_password.unwrap_or_default(),
            oauth_token_secret.unwrap_or_default(),
        )))
    } else {
        None
    };

    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let profile_id = workspace_id.to_string();
    let handle = tauri::async_runtime::spawn(async move {
        let result = serve(
            actions_port,
            &profile_id,
            workspace_path,
            public_base_url,
            auth_type,
            api_key,
            oauth,
            oauth_base,
            oauth_client_secret,
            policy,
            shutdown_rx,
        )
        .await;
        if let Err(err) = &result {
            append_profile_log(
                &profile_id,
                "actions-stderr.log",
                &format!("[actions] listener stopped: {err}"),
            );
            eprintln!("actions listener stopped: {err}");
        } else {
            append_profile_log(
                &profile_id,
                "actions-stderr.log",
                "[actions] listener stopped",
            );
        }
    });
    Ok((shutdown_tx, handle))
}

async fn serve(
    actions_port: u16,
    profile_id: &str,
    workspace_path: PathBuf,
    public_base_url: String,
    auth_type: String,
    api_key: Option<String>,
    oauth: Option<Arc<OAuthRuntime>>,
    oauth_base_url: String,
    oauth_client_secret: Option<String>,
    policy: PolicySettings,
    shutdown: oneshot::Receiver<()>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let workspace = tools::Workspace::new(workspace_path.clone()).map_err(|e| e.message())?;
    let ctx = Arc::new(ToolContext::from_workspace(
        workspace,
        crate::workspace::AuthConfig {
            auth_type: auth_type.clone(),
            ..crate::workspace::AuthConfig::default()
        },
        policy.clone(),
        "full".into(),
        policy.permission_mode.clone(),
    ));
    let tools: Vec<Value> = tools::list_tools()
        .into_iter()
        .filter(|tool| {
            tool.get("name")
                .and_then(Value::as_str)
                .map(is_allowed_tool)
                .unwrap_or(false)
        })
        .collect();
    let openapi_doc = openapi::build_openapi(&tools, &public_base_url, &auth_type);

    let auth = Arc::new(AuthConfig::new(auth_type, api_key, oauth.clone()));

    let state = AppState {
        workspace_path: ctx.workspace_path(),
        ctx,
        openapi: Arc::new(RwLock::new(openapi_doc)),
        auth: auth.clone(),
        oauth,
        oauth_base_url,
        oauth_client_secret,
        write_lock: Arc::new(Mutex::new(())),
    };

    let protected = Router::new()
        .route("/actions/{tool_name}", post(execute_action))
        .layer(middleware::from_fn(require_actions_auth))
        .layer(Extension(auth));

    let app = Router::new()
        .route("/health", get(health))
        .route("/openapi.json", get(openapi_json))
        .route("/privacy", get(privacy))
        .route(
            "/.well-known/oauth-authorization-server",
            get(oauth_authorization_server_metadata),
        )
        .route("/oauth/authorize", get(oauth_authorize_get).post(oauth_authorize_post))
        .route("/oauth/token", post(oauth_token_post))
        .merge(protected)
        .with_state(state)
        .layer(CorsLayer::permissive());

    let addr = SocketAddr::from(([127, 0, 0, 1], actions_port));
    let listener = tokio::net::TcpListener::bind(addr).await?;
    append_profile_log(
        profile_id,
        "actions-stdout.log",
        &format!(
            "[actions] listening on http://127.0.0.1:{actions_port} (public: {public_base_url})"
        ),
    );
    axum::serve(listener, app)
        .with_graceful_shutdown(async {
            let _ = shutdown.await;
        })
        .await?;
    Ok(())
}

async fn health(State(state): State<AppState>) -> Json<Value> {
    let tools_loaded = state
        .openapi
        .read()
        .await
        .get("paths")
        .and_then(Value::as_object)
        .map(|paths| paths.len())
        .unwrap_or(0);

    Json(json!({
        "ok": true,
        "service": "coding-tools-actions",
        "workspace": state.workspace_path,
        "auth_type": state.auth.auth_type,
        "tools_loaded": tools_loaded
    }))
}

async fn openapi_json(State(state): State<AppState>) -> Json<Value> {
    Json(state.openapi.read().await.clone())
}

async fn privacy() -> Html<&'static str> {
    Html(
        r#"<!doctype html>
<html lang="zh-CN">
  <head>
    <meta charset="utf-8">
    <title>Coding Tools Actions Privacy</title>
  </head>
  <body>
    <h1>隐私政策</h1>
    <p>本服务仅供仓库所有者本人使用。</p>
    <p>请求内容只用于执行用户主动发起的代码操作。</p>
    <p>服务不会出售或共享请求数据。</p>
    <p>API 密钥、GitHub 令牌和环境变量不会返回给模型。</p>
  </body>
</html>"#,
    )
}

async fn oauth_authorization_server_metadata(State(state): State<AppState>) -> Response {
    if !state.auth.oauth_enabled() {
        return oauth_not_configured();
    }
    Json(authorization_server_metadata(
        &state.oauth_base_url,
        state.oauth_client_secret.as_deref(),
    ))
    .into_response()
}

async fn oauth_authorize_get(
    State(state): State<AppState>,
    Query(params): Query<AuthorizeParams>,
) -> Response {
    let Some(oauth) = state.oauth.as_ref() else {
        return oauth_not_configured();
    };
    authorize_get(oauth, params)
}

async fn oauth_authorize_post(
    State(state): State<AppState>,
    Form(form): Form<AuthorizeForm>,
) -> Response {
    let Some(oauth) = state.oauth.as_ref() else {
        return oauth_not_configured();
    };
    authorize_post(oauth, form)
}

async fn oauth_token_post(
    State(state): State<AppState>,
    headers: HeaderMap,
    Form(form): Form<TokenForm>,
) -> Response {
    let Some(oauth) = state.oauth.as_ref() else {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "unsupported_grant_type" })),
        )
            .into_response();
    };
    token_exchange(oauth, &headers, form)
}

fn oauth_not_configured() -> Response {
    (
        StatusCode::NOT_FOUND,
        Json(json!({ "error": "OAuth not configured" })),
    )
        .into_response()
}

async fn execute_action(
    State(state): State<AppState>,
    Path(tool_name): Path<String>,
    body: Option<Json<Value>>,
) -> Response {
    let arguments = match body {
        Some(Json(value)) if value.is_object() || value.is_null() => {
            if value.is_null() {
                json!({})
            } else {
                value
            }
        }
        Some(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({ "detail": "Request body must be a JSON object" })),
            )
                .into_response();
        }
        None => json!({}),
    };

    if let Err(err) = tools::policy::validate_actions_exposure(&tool_name) {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "detail": err.to_string() })),
        )
            .into_response();
    }

    let structured = if tools::registry::MUTATING_TOOLS.contains(&tool_name.as_str()) {
        let _guard = state.write_lock.lock().await;
        tools::call_tool(state.ctx.as_ref(), &tool_name, &arguments)
    } else {
        tools::call_tool(state.ctx.as_ref(), &tool_name, &arguments)
    };
    let result = wrap_tool_result(structured);
    let is_error = result
        .get("isError")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let status = if is_error {
        StatusCode::UNPROCESSABLE_ENTITY
    } else {
        StatusCode::OK
    };
    (
        status,
        Json(json!({
            "ok": !is_error,
            "tool": tool_name,
            "structured_content": result.get("structuredContent").cloned().unwrap_or(Value::Null),
            "content": result.get("content").cloned().unwrap_or_else(|| json!([])),
            "is_error": is_error
        })),
    )
        .into_response()
}
