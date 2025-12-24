//! Shared test utilities for CLI tests.

use std::sync::{Arc, Mutex};

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
        self.captured
            .lock()
            .expect("captured mutex should be available")
            .replace((locator.clone(), params.clone()));

        self.response
            .lock()
            .expect("response mutex should be available")
            .take()
            .expect("response should only be consumed once")
    }
}
