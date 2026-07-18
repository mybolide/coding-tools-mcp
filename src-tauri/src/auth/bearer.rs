use axum::http::{
    header::{AUTHORIZATION, WWW_AUTHENTICATE},
    HeaderMap, HeaderValue, StatusCode,
};
use axum::response::{IntoResponse, Response};

pub fn verify_bearer_header(
    headers: &HeaderMap,
    expected: &str,
    resource_metadata_url: &str,
) -> Option<Response> {
    let Some(header_value) = headers.get(AUTHORIZATION) else {
        return Some(unauthorized_response(
            "Missing Authorization header",
            Some(resource_metadata_url),
        ));
    };

    let Ok(header_str) = header_value.to_str() else {
        return Some(unauthorized_response(
            "Invalid Authorization header",
            Some(resource_metadata_url),
        ));
    };

    let Some(token) = header_str.strip_prefix("Bearer ").map(str::trim) else {
        return Some(unauthorized_response(
            "Invalid bearer token",
            Some(resource_metadata_url),
        ));
    };

    if !constant_time_eq_str(token, expected) {
        return Some(unauthorized_response(
            "Invalid bearer token",
            Some(resource_metadata_url),
        ));
    }

    None
}

pub(crate) fn unauthorized_response(
    message: &str,
    resource_metadata_url: Option<&str>,
) -> Response {
    let mut response = (StatusCode::UNAUTHORIZED, message.to_string()).into_response();
    let mut challenge = String::from("Bearer realm=\"coding-tools-mcp\"");
    if let Some(url) = resource_metadata_url.filter(|value| !value.is_empty()) {
        challenge.push_str(", resource_metadata=\"");
        challenge.push_str(&escape_quoted_value(url));
        challenge.push('"');
    }
    if let Ok(value) = HeaderValue::from_str(&challenge) {
        response.headers_mut().insert(WWW_AUTHENTICATE, value);
    }
    response
}

fn escape_quoted_value(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace(['\r', '\n'], "")
}

pub(crate) fn constant_time_eq_str(left: &str, right: &str) -> bool {
    constant_time_eq(left.as_bytes(), right.as_bytes())
}

fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    left.iter()
        .zip(right.iter())
        .fold(0u8, |acc, (a, b)| acc | (a ^ b))
        == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_valid_bearer_token() {
        let mut headers = HeaderMap::new();
        headers.insert(AUTHORIZATION, "Bearer secret-token".parse().unwrap());
        assert!(
            verify_bearer_header(&headers, "secret-token", "https://example.com/meta").is_none()
        );
    }

    #[test]
    fn rejects_missing_or_invalid_bearer_token() {
        let headers = HeaderMap::new();
        let response = verify_bearer_header(&headers, "secret-token", "https://example.com/meta")
            .expect("missing token should be rejected");
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        assert_eq!(
            response.headers().get(WWW_AUTHENTICATE).unwrap(),
            "Bearer realm=\"coding-tools-mcp\", resource_metadata=\"https://example.com/meta\""
        );

        let mut headers = HeaderMap::new();
        headers.insert(AUTHORIZATION, "Basic secret-token".parse().unwrap());
        assert!(
            verify_bearer_header(&headers, "secret-token", "https://example.com/meta").is_some()
        );

        let mut headers = HeaderMap::new();
        headers.insert(AUTHORIZATION, "Bearer wrong".parse().unwrap());
        assert!(
            verify_bearer_header(&headers, "secret-token", "https://example.com/meta").is_some()
        );
    }
}
