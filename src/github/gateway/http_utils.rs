//! Shared HTTP utilities for gateway implementations.

use http::header::{HeaderMap, HeaderValue, IF_MODIFIED_SINCE, IF_NONE_MATCH};

use crate::persistence::CachedPullRequestMetadata;

pub(super) fn build_conditional_headers(cached: &CachedPullRequestMetadata) -> Option<HeaderMap> {
    let mut headers = HeaderMap::new();

    if let Some(etag) = cached.etag.as_deref()
        && let Ok(value) = etag.parse()
    {
        headers.insert(IF_NONE_MATCH, value);
    }

    if let Some(last_modified) = cached.last_modified.as_deref()
        && let Ok(value) = last_modified.parse()
    {
        headers.insert(IF_MODIFIED_SINCE, value);
    }

    if headers.is_empty() {
        None
    } else {
        Some(headers)
    }
}

pub(super) fn header_to_string(header_value: Option<&HeaderValue>) -> Option<String> {
    header_value
        .and_then(|raw| raw.to_str().ok())
        .map(ToOwned::to_owned)
}

pub(super) fn extract_github_message(body: &str) -> Option<String> {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(body) else {
        return None;
    };
    value
        .get("message")
        .and_then(serde_json::Value::as_str)
        .map(ToOwned::to_owned)
}
