use std::sync::Arc;

use axum::{
    extract::Request,
    http::{header::AUTHORIZATION, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Extension,
};

use crate::auth::{verify_oauth_bearer_header, OAuthRuntime};

use super::bearer::constant_time_eq;

#[derive(Clone)]
pub struct AuthConfig {
    pub auth_type: String,
    pub api_key: Option<String>,
    pub oauth: Option<Arc<OAuthRuntime>>,
}

impl AuthConfig {
    pub fn new(auth_type: String, api_key: Option<String>, oauth: Option<Arc<OAuthRuntime>>) -> Self {
        Self {
            auth_type,
            api_key,
            oauth,
        }
    }

    pub fn no_auth(&self) -> bool {
        self.auth_type == "none"
    }

    pub fn api_key_enabled(&self) -> bool {
        self.auth_type == "api_key"
    }

    pub fn oauth_enabled(&self) -> bool {
        self.auth_type == "oauth"
    }
}

pub async fn require_actions_auth(
    Extension(auth): Extension<Arc<AuthConfig>>,
    request: Request,
    next: Next,
) -> Response {
    if auth.no_auth() {
        return next.run(request).await;
    }

    if auth.api_key_enabled() {
        let Some(expected) = auth.api_key.as_ref() else {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Actions API key is not configured",
            )
                .into_response();
        };

        let Some(header_value) = request.headers().get(AUTHORIZATION) else {
            return (StatusCode::UNAUTHORIZED, "Missing Authorization header").into_response();
        };

        let Ok(header_str) = header_value.to_str() else {
            return (StatusCode::UNAUTHORIZED, "Invalid Authorization header").into_response();
        };

        let Some(token) = header_str.strip_prefix("Bearer ").map(str::trim) else {
            return (StatusCode::UNAUTHORIZED, "Invalid API key").into_response();
        };

        if !constant_time_eq(token.as_bytes(), expected.as_bytes()) {
            return (StatusCode::UNAUTHORIZED, "Invalid API key").into_response();
        }

        return next.run(request).await;
    }

    if auth.oauth_enabled() {
        let Some(oauth) = auth.oauth.as_ref() else {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Actions OAuth is not configured",
            )
                .into_response();
        };
        if let Some(response) = verify_oauth_bearer_header(request.headers(), oauth) {
            return response;
        }
        return next.run(request).await;
    }

    (
        StatusCode::INTERNAL_SERVER_ERROR,
        format!("Unsupported Actions auth type: {}", auth.auth_type),
    )
        .into_response()
}
