use std::path::PathBuf;
use std::sync::Arc;

use axum::extract::{Form, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde_json::{json, Value};
use tokio::sync::oneshot;
use tower_http::cors::CorsLayer;

use crate::auth::{
    authorization_server_metadata, authorize_get, authorize_post, external_base_url,
    protected_resource_metadata, token_exchange, verify_bearer_header, verify_oauth_bearer_header,
    AuthorizeForm, AuthorizeParams, OAuthRuntime, TokenForm,
};
use crate::mcp::server::{handle_request, new_state, SharedState};
use crate::secret::SecretStore;
use crate::tools::Workspace;
use crate::tunnel::append_profile_log;
use crate::tools::policy::PolicySettings;
use crate::workspace::{AuthConfig, RuntimeConfig};

pub type ShutdownSender = oneshot::Sender<()>;

#[derive(Clone)]
struct ListenerState {
    mcp: SharedState,
    auth: AuthConfig,
    workspace_id: String,
    workspace_path: String,
    bind_port: u16,
    configured_public_url: String,
    bearer_token: Option<String>,
    oauth: Option<Arc<OAuthRuntime>>,
    oauth_client_secret: Option<String>,
}

#[allow(clippy::too_many_arguments)]
pub fn spawn_listener(
    port: u16,
    workspace_path: PathBuf,
    workspace_id: String,
    auth: AuthConfig,
    public_base_url: String,
    oauth_client_secret: Option<String>,
    oauth_password: Option<String>,
    oauth_token_secret: Option<String>,
    runtime: RuntimeConfig,
) -> Result<(ShutdownSender, tauri::async_runtime::JoinHandle<()>), String> {
    let workspace_display = workspace_path.display().to_string();
    let workspace = Workspace::new(workspace_path).map_err(|e| e.message())?;
    let policy = PolicySettings::from_runtime(&runtime);
    let mcp = new_state(
        workspace,
        auth.clone(),
        policy,
        runtime.tool_profile.clone(),
        runtime.permission_mode.clone(),
    );
    let bearer_token = if auth.bearer_enabled() {
        let key = "bearer_token";
        if auth.use_shared_secrets {
            SecretStore::get_shared(key).map_err(|e| e.to_string())?
        } else {
            SecretStore::get(&workspace_id, key).map_err(|e| e.to_string())?
        }
    } else {
        None
    };
    let configured_public_url = public_base_url.trim().to_string();
    let oauth = if auth.oauth_enabled() {
        let password = oauth_password.unwrap_or_default();
        let token_secret = oauth_token_secret.unwrap_or_default();
        let oauth_base = external_base_url(
            &HeaderMap::new(),
            port,
            &configured_public_url,
        );
        Some(Arc::new(OAuthRuntime::new(
            oauth_base,
            auth.oauth_client_id.clone(),
            oauth_client_secret.clone(),
            password,
            token_secret,
        )))
    } else {
        None
    };
    let state = ListenerState {
        mcp,
        auth,
        workspace_id,
        workspace_path: workspace_display,
        bind_port: port,
        configured_public_url,
        bearer_token,
        oauth,
        oauth_client_secret,
    };
    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let profile_id = state.workspace_id.clone();
    let handle = tauri::async_runtime::spawn(async move {
        let result = serve(port, state, shutdown_rx).await;
        if let Err(err) = &result {
            append_profile_log(
                &profile_id,
                "stderr.log",
                &format!("[mcp] listener stopped: {err}"),
            );
            eprintln!("mcp listener stopped: {err}");
        } else {
            append_profile_log(&profile_id, "stderr.log", "[mcp] listener stopped");
        }
    });
    Ok((shutdown_tx, handle))
}

async fn serve(
    port: u16,
    state: ListenerState,
    shutdown: oneshot::Receiver<()>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let profile_id = state.workspace_id.clone();
    let app = Router::new()
        .route("/mcp", get(mcp_discovery).post(mcp_post))
        .route(
            "/.well-known/oauth-authorization-server",
            get(oauth_authorization_server_metadata),
        )
        .route(
            "/.well-known/oauth-protected-resource",
            get(oauth_protected_resource_metadata),
        )
        .route("/oauth/authorize", get(oauth_authorize_get).post(oauth_authorize_post))
        .route("/oauth/token", post(oauth_token_post))
        .with_state(state)
        .layer(CorsLayer::permissive());

    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], port));
    let listener = tokio::net::TcpListener::bind(addr).await?;
    append_profile_log(
        &profile_id,
        "stdout.log",
        &format!("[mcp] listening on http://127.0.0.1:{port}/mcp"),
    );
    axum::serve(listener, app)
        .with_graceful_shutdown(async {
            let _ = shutdown.await;
        })
        .await?;
    Ok(())
}

async fn mcp_discovery() -> Json<Value> {
    Json(json!({
        "name": "coding-tools-mcp",
        "version": "0.1.0",
        "protocolVersion": "2025-06-18"
    }))
}

fn resolve_oauth_base(state: &ListenerState, headers: &HeaderMap) -> String {
    external_base_url(headers, state.bind_port, &state.configured_public_url)
}

async fn mcp_post(
    State(state): State<ListenerState>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Response {
    if let Some(response) = require_mcp_auth(&state, &headers) {
        return response;
    }
    Json(handle_request(&state.mcp, &body)).into_response()
}

fn require_mcp_auth(state: &ListenerState, headers: &HeaderMap) -> Option<Response> {
    if state.auth.bearer_enabled() {
        let expected = state.bearer_token.as_deref().unwrap_or("");
        return verify_bearer_header(headers, expected);
    }
    if state.auth.oauth_enabled() {
        if let Some(oauth) = state.oauth.as_ref() {
            let server_url = resolve_oauth_base(state, headers);
            return verify_oauth_bearer_header(headers, oauth, &server_url);
        }
    }
    None
}

async fn oauth_authorization_server_metadata(
    State(state): State<ListenerState>,
    headers: HeaderMap,
) -> Response {
    if !state.auth.oauth_enabled() {
        return oauth_not_configured();
    }
    let base = resolve_oauth_base(&state, &headers);
    Json(authorization_server_metadata(
        &base,
        state.oauth_client_secret.as_deref(),
    ))
    .into_response()
}

async fn oauth_protected_resource_metadata(
    State(state): State<ListenerState>,
    headers: HeaderMap,
) -> Response {
    if !state.auth.oauth_enabled() {
        return oauth_not_configured();
    }
    Json(protected_resource_metadata(&resolve_oauth_base(&state, &headers))).into_response()
}

async fn oauth_authorize_get(
    State(state): State<ListenerState>,
    Query(params): Query<AuthorizeParams>,
) -> Response {
    let Some(oauth) = state.oauth.as_ref() else {
        return oauth_not_configured();
    };
    authorize_get(
        oauth,
        params,
        Some(state.workspace_path.as_str()),
    )
}

async fn oauth_authorize_post(
    State(state): State<ListenerState>,
    headers: HeaderMap,
    Form(form): Form<AuthorizeForm>,
) -> Response {
    let Some(oauth) = state.oauth.as_ref() else {
        return oauth_not_configured();
    };
    authorize_post(oauth, form, &resolve_oauth_base(&state, &headers))
}

async fn oauth_token_post(
    State(state): State<ListenerState>,
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
    token_exchange(
        oauth,
        &headers,
        form,
        &resolve_oauth_base(&state, &headers),
    )
}

fn oauth_not_configured() -> Response {
    (
        StatusCode::NOT_FOUND,
        Json(json!({ "error": "OAuth not configured" })),
    )
        .into_response()
}
