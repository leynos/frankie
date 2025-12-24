//! Tests for `run_discovered_repository_with_gateway_builder`.

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use frankie::github::{PageInfo, RepositoryGateway};
use frankie::local::GitHubOrigin;
use frankie::{
    FrankieConfig, IntakeError, ListPullRequestsParams, PaginatedPullRequests, PullRequestState,
    RepositoryLocator,
};

use super::super::run_discovered_repository_with_gateway_builder;

#[derive(Clone)]
struct CapturingGateway {
    captured: Arc<Mutex<Option<(RepositoryLocator, ListPullRequestsParams)>>>,
    response: Arc<Mutex<Option<Result<PaginatedPullRequests, IntakeError>>>>,
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

#[tokio::test]
async fn extracts_owner_repo_from_github_origin_and_wires_gateway() {
    let config = FrankieConfig {
        token: Some("ghp_test_token".to_owned()),
        ..Default::default()
    };
    let origin = GitHubOrigin::GitHubCom {
        owner: "discovered-owner".to_owned(),
        repository: "discovered-repo".to_owned(),
    };

    let captured = Arc::new(Mutex::new(None));
    let gateway = CapturingGateway {
        captured: Arc::clone(&captured),
        response: Arc::new(Mutex::new(Some(Ok(PaginatedPullRequests {
            items: vec![],
            page_info: PageInfo::default(),
            rate_limit: None,
        })))),
    };

    let mut buffer = Vec::new();
    run_discovered_repository_with_gateway_builder(
        &config,
        &origin,
        move |token, locator| {
            assert_eq!(
                token.value(),
                "ghp_test_token",
                "token should be passed to gateway builder"
            );
            assert_eq!(
                locator.owner().as_str(),
                "discovered-owner",
                "locator owner should come from GitHubOrigin"
            );
            assert_eq!(
                locator.repository().as_str(),
                "discovered-repo",
                "locator repo should come from GitHubOrigin"
            );
            assert_eq!(
                locator.api_base().as_str(),
                "https://api.github.com/",
                "GitHubCom origin should use github.com API"
            );
            Ok(gateway)
        },
        &mut buffer,
    )
    .await
    .expect("run_discovered_repository should succeed");

    // Verify gateway was called with correct locator and default params
    let (locator, params) = captured
        .lock()
        .expect("captured mutex should be available")
        .clone()
        .expect("gateway should have been called");
    assert_eq!(locator.owner().as_str(), "discovered-owner");
    assert_eq!(locator.repository().as_str(), "discovered-repo");
    assert_eq!(params.state, Some(PullRequestState::All));
    assert_eq!(params.per_page, Some(50));
    assert_eq!(params.page, Some(1));

    // Verify output contains owner/repo from GitHubOrigin
    let output = String::from_utf8(buffer).expect("output should be valid UTF-8");
    assert!(
        output.contains("Pull requests for discovered-owner/discovered-repo:"),
        "output should use owner/repo from GitHubOrigin: {output}"
    );
}

#[tokio::test]
async fn uses_enterprise_api_for_enterprise_origin() {
    let config = FrankieConfig {
        token: Some("ghp_enterprise".to_owned()),
        ..Default::default()
    };
    let origin = GitHubOrigin::Enterprise {
        host: "ghe.corp.example.com".to_owned(),
        port: None,
        owner: "corp-org".to_owned(),
        repository: "internal-project".to_owned(),
    };

    let gateway = CapturingGateway {
        captured: Arc::new(Mutex::new(None)),
        response: Arc::new(Mutex::new(Some(Ok(PaginatedPullRequests {
            items: vec![],
            page_info: PageInfo::default(),
            rate_limit: None,
        })))),
    };

    let mut buffer = Vec::new();
    run_discovered_repository_with_gateway_builder(
        &config,
        &origin,
        move |_token, locator| {
            assert!(
                locator
                    .api_base()
                    .as_str()
                    .starts_with("https://ghe.corp.example.com/api/v3"),
                "Enterprise origin should use Enterprise API: {}",
                locator.api_base()
            );
            Ok(gateway)
        },
        &mut buffer,
    )
    .await
    .expect("run_discovered_repository should succeed for enterprise");
}

#[tokio::test]
async fn propagates_token_error_when_missing() {
    // Ensure GITHUB_TOKEN is not set (resolve_token falls back to env var)
    let _guard = env_lock::lock_env([("GITHUB_TOKEN", None::<&str>)]);

    let config = FrankieConfig {
        token: None,
        ..Default::default()
    };
    let origin = GitHubOrigin::GitHubCom {
        owner: "owner".to_owned(),
        repository: "repo".to_owned(),
    };

    let mut buffer = Vec::new();
    let result = run_discovered_repository_with_gateway_builder(
        &config,
        &origin,
        |_token, _locator| -> Result<CapturingGateway, IntakeError> {
            panic!("gateway builder should not be called when token is missing")
        },
        &mut buffer,
    )
    .await;

    assert!(
        matches!(result, Err(IntakeError::MissingToken)),
        "missing token should return MissingToken error: {result:?}"
    );
}

#[tokio::test]
async fn propagates_gateway_errors() {
    let config = FrankieConfig {
        token: Some("ghp_test".to_owned()),
        ..Default::default()
    };
    let origin = GitHubOrigin::GitHubCom {
        owner: "owner".to_owned(),
        repository: "repo".to_owned(),
    };

    let gateway = CapturingGateway {
        captured: Arc::new(Mutex::new(None)),
        response: Arc::new(Mutex::new(Some(Err(IntakeError::Api {
            message: "Not Found".to_owned(),
        })))),
    };

    let mut buffer = Vec::new();
    let result = run_discovered_repository_with_gateway_builder(
        &config,
        &origin,
        |_token, _locator| Ok(gateway),
        &mut buffer,
    )
    .await;

    assert!(
        matches!(result, Err(IntakeError::Api { .. })),
        "gateway errors should be propagated: {result:?}"
    );
}
