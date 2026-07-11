use axum::http::{header::AUTHORIZATION, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};

pub fn verify_bearer_header(headers: &HeaderMap, expected: &str) -> Option<Response> {
    let Some(header_value) = headers.get(AUTHORIZATION) else {
        return Some((StatusCode::UNAUTHORIZED, "Missing Authorization header").into_response());
    };

    let Ok(header_str) = header_value.to_str() else {
        return Some((StatusCode::UNAUTHORIZED, "Invalid Authorization header").into_response());
    };

    let Some(token) = header_str.strip_prefix("Bearer ").map(str::trim) else {
        return Some((StatusCode::UNAUTHORIZED, "Invalid bearer token").into_response());
    };

    if !constant_time_eq_str(token, expected) {
        return Some((StatusCode::UNAUTHORIZED, "Invalid bearer token").into_response());
    }

    None
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
        assert!(verify_bearer_header(&headers, "secret-token").is_none());
    }

    #[test]
    fn rejects_missing_or_invalid_bearer_token() {
        let headers = HeaderMap::new();
        assert!(verify_bearer_header(&headers, "secret-token").is_some());

        let mut headers = HeaderMap::new();
        headers.insert(AUTHORIZATION, "Basic secret-token".parse().unwrap());
        assert!(verify_bearer_header(&headers, "secret-token").is_some());

        let mut headers = HeaderMap::new();
        headers.insert(AUTHORIZATION, "Bearer wrong".parse().unwrap());
        assert!(verify_bearer_header(&headers, "secret-token").is_some());
    }
}
