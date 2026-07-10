//! Shared test utilities for CLI tests.

use std::sync::{Arc, Mutex, PoisonError};

use async_trait::async_trait;
use frankie::github::RepositoryGateway;
use frankie::{IntakeError, ListPullRequestsParams, PaginatedPullRequests, RepositoryLocator};

/// A mock gateway that captures its inputs and returns a preconfigured response.
#[derive(Clone)]
pub struct CapturingGateway {
    /// Captured locator and params from the last call.
    pub captured: Arc<Mutex<Option<(RepositoryLocator, ListPullRequestsParams)>>>,
    /// Response to return (consumed on first call).
    pub response: Arc<Mutex<Option<Result<PaginatedPullRequests, IntakeError>>>>,
}

#[async_trait]
impl RepositoryGateway for CapturingGateway {
    async fn list_pull_requests(
        &self,
        locator: &RepositoryLocator,
        params: &ListPullRequestsParams,
    ) -> Result<PaginatedPullRequests, IntakeError> {
        // Poisoned mutexes are recovered: the mock stores plain data that
        // stays valid after a panic in another thread.
        self.captured
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .replace((locator.clone(), params.clone()));

        self.response
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take()
            .unwrap_or_else(|| {
                Err(IntakeError::Api {
                    message: "mock response was already consumed".to_owned(),
                })
            })
    }
}
