//! Octocrab gateway that caches pull request metadata in `SQLite`.

use async_trait::async_trait;
use http::header::{ETAG, LAST_MODIFIED};
use http::{StatusCode, Uri};
use octocrab::{Octocrab, Page};

use crate::github::error::IntakeError;
use crate::github::locator::{PersonalAccessToken, PullRequestLocator};
use crate::github::models::{ApiComment, ApiPullRequest, PullRequestComment, PullRequestMetadata};
use crate::persistence::{
    CachedPullRequestMetadata, PullRequestMetadataCache, PullRequestMetadataCacheWrite,
};

use super::PullRequestGateway;
use super::client::build_octocrab_client;
use super::error_mapping::{map_http_error, map_octocrab_error, map_persistence_error};
use super::http_utils::{build_conditional_headers, extract_github_message, header_to_string};

#[derive(Debug, Clone)]
struct ResponseValidators {
    etag: Option<String>,
    last_modified: Option<String>,
}

/// Octocrab-backed gateway that caches pull request metadata in `SQLite`.
///
/// Only the metadata call is cached; comment listing currently always calls the
/// GitHub API.
pub struct OctocrabCachingGateway {
    client: Octocrab,
    cache: PullRequestMetadataCache,
    ttl_seconds: u64,
}

impl OctocrabCachingGateway {
    /// Builds a caching gateway for the given token, pull request locator, and
    /// database URL.
    ///
    /// # Errors
    ///
    /// Returns [`IntakeError`] when the Octocrab client cannot be constructed
    /// or when the database URL is invalid.
    pub fn for_token(
        token: &PersonalAccessToken,
        locator: &PullRequestLocator,
        database_url: &str,
        ttl_seconds: u64,
    ) -> Result<Self, IntakeError> {
        let octocrab = build_octocrab_client(token, locator.api_base().as_str())?;
        let cache = PullRequestMetadataCache::new(database_url.to_owned())
            .map_err(|error| map_persistence_error("initialise cache", &error))?;
        Ok(Self {
            client: octocrab,
            cache,
            ttl_seconds,
        })
    }

    fn expiry_window(&self, now_unix: i64) -> (i64, i64) {
        let ttl_unix = i64::try_from(self.ttl_seconds).unwrap_or(i64::MAX);
        let expires_at = now_unix.saturating_add(ttl_unix);
        (now_unix, expires_at)
    }

    async fn fetch_pull_request(
        &self,
        locator: &PullRequestLocator,
        conditional: Option<&CachedPullRequestMetadata>,
    ) -> Result<FetchResult, IntakeError> {
        let headers = conditional.and_then(build_conditional_headers);
        let uri: Uri = locator
            .pull_request_path()
            .parse::<Uri>()
            .map_err(|error| IntakeError::InvalidUrl(error.to_string()))?;

        let response = self
            .client
            ._get_with_headers(uri, headers)
            .await
            .map_err(|error| map_octocrab_error("pull request", &error))?;

        match response.status() {
            StatusCode::NOT_MODIFIED => Ok(FetchResult::NotModified),
            StatusCode::OK => {
                let validators = ResponseValidators {
                    etag: header_to_string(response.headers().get(ETAG)),
                    last_modified: header_to_string(response.headers().get(LAST_MODIFIED)),
                };

                let body = self
                    .client
                    .body_to_string(response)
                    .await
                    .map_err(|error| IntakeError::Api {
                        message: format!("pull request response decode failed: {error}"),
                    })?;

                let api: ApiPullRequest =
                    serde_json::from_str(&body).map_err(|error| IntakeError::Api {
                        message: format!("pull request response deserialisation failed: {error}"),
                    })?;

                Ok(FetchResult::Modified {
                    metadata: api.into(),
                    validators,
                })
            }
            status => {
                let body = self
                    .client
                    .body_to_string(response)
                    .await
                    .unwrap_or_else(|_| String::new());

                Err(map_http_error(
                    "pull request",
                    status,
                    extract_github_message(&body),
                ))
            }
        }
    }
}

enum FetchResult {
    NotModified,
    Modified {
        metadata: PullRequestMetadata,
        validators: ResponseValidators,
    },
}

#[async_trait]
impl PullRequestGateway for OctocrabCachingGateway {
    async fn pull_request(
        &self,
        locator: &PullRequestLocator,
    ) -> Result<PullRequestMetadata, IntakeError> {
        let now = PullRequestMetadataCache::now_unix_seconds();
        let cached = self
            .cache
            .get(locator)
            .map_err(|error| map_persistence_error("read cache", &error))?;

        if let Some(entry) = cached {
            if !entry.is_expired(now) {
                return Ok(entry.metadata);
            }

            match self.fetch_pull_request(locator, Some(&entry)).await? {
                FetchResult::NotModified => {
                    let (fetched_at, expires_at) = self.expiry_window(now);
                    self.cache
                        .touch(locator, fetched_at, expires_at)
                        .map_err(|error| map_persistence_error("update cache", &error))?;
                    Ok(entry.metadata)
                }
                FetchResult::Modified {
                    metadata,
                    validators,
                } => {
                    let (fetched_at, expires_at) = self.expiry_window(now);
                    self.cache
                        .upsert(
                            locator,
                            PullRequestMetadataCacheWrite {
                                metadata: &metadata,
                                etag: validators.etag.as_deref(),
                                last_modified: validators.last_modified.as_deref(),
                                fetched_at_unix: fetched_at,
                                expires_at_unix: expires_at,
                            },
                        )
                        .map_err(|error| map_persistence_error("write cache", &error))?;
                    Ok(metadata)
                }
            }
        } else {
            match self.fetch_pull_request(locator, None).await? {
                FetchResult::NotModified => Err(IntakeError::Api {
                    message: "unexpected 304 for uncached pull request".to_owned(),
                }),
                FetchResult::Modified {
                    metadata,
                    validators,
                } => {
                    let (fetched_at, expires_at) = self.expiry_window(now);
                    self.cache
                        .upsert(
                            locator,
                            PullRequestMetadataCacheWrite {
                                metadata: &metadata,
                                etag: validators.etag.as_deref(),
                                last_modified: validators.last_modified.as_deref(),
                                fetched_at_unix: fetched_at,
                                expires_at_unix: expires_at,
                            },
                        )
                        .map_err(|error| map_persistence_error("write cache", &error))?;
                    Ok(metadata)
                }
            }
        }
    }

    async fn pull_request_comments(
        &self,
        locator: &PullRequestLocator,
    ) -> Result<Vec<PullRequestComment>, IntakeError> {
        let page = self
            .client
            .get::<Page<ApiComment>, _, _>(locator.comments_path(), None::<&()>)
            .await
            .map_err(|error| map_octocrab_error("issue comments", &error))?;

        self.client
            .all_pages(page)
            .await
            .map(|comments| comments.into_iter().map(ApiComment::into).collect())
            .map_err(|error| map_octocrab_error("issue comments", &error))
    }
}
