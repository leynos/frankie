//! Repository pull request listing operation.

use std::io::{self, Write};

use frankie::github::RepositoryGateway;
use frankie::{
    FrankieConfig, IntakeError, ListPullRequestsParams, OctocrabRepositoryGateway,
    PersonalAccessToken, PullRequestState, RepositoryIntake, RepositoryLocator,
};

use super::output::write_listing_summary;

/// Lists pull requests for a repository.
///
/// # Errors
///
/// Returns [`IntakeError::Configuration`] if required configuration is missing.
/// Returns [`IntakeError::GitHub`] if the API request fails.
pub async fn run(config: &FrankieConfig) -> Result<(), IntakeError> {
    let mut stdout = io::stdout().lock();
    run_with_gateway_builder(config, OctocrabRepositoryGateway::for_token, &mut stdout).await
}

/// Lists pull requests using a custom gateway builder.
///
/// This function is exposed for testing with mock gateways.
pub async fn run_with_gateway_builder<G, F, W>(
    config: &FrankieConfig,
    build_gateway: F,
    writer: &mut W,
) -> Result<(), IntakeError>
where
    G: RepositoryGateway,
    F: FnOnce(&PersonalAccessToken, &RepositoryLocator) -> Result<G, IntakeError>,
    W: Write,
{
    let (owner, repo) = config.require_repository_info()?;
    let token_value = config.resolve_token()?;

    let locator = RepositoryLocator::from_owner_repo(owner, repo)?;
    let token = PersonalAccessToken::new(token_value)?;

    let gateway = build_gateway(&token, &locator)?;
    let intake = RepositoryIntake::new(&gateway);

    let result = intake
        .list_pull_requests(&locator, &default_listing_params())
        .await?;
    write_listing_summary(writer, &result, owner, repo)
}

/// Returns the default parameters for listing pull requests.
const fn default_listing_params() -> ListPullRequestsParams {
    ListPullRequestsParams {
        state: Some(PullRequestState::All),
        per_page: Some(50),
        page: Some(1),
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use async_trait::async_trait;
    use frankie::github::{PageInfo, RepositoryGateway};
    use frankie::{
        FrankieConfig, IntakeError, ListPullRequestsParams, PaginatedPullRequests,
        PullRequestState, RepositoryLocator,
    };

    use super::run_with_gateway_builder;

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
    async fn run_repository_listing_uses_expected_params_and_writes_output() {
        let config = FrankieConfig {
            token: Some("ghp_example".to_owned()),
            owner: Some("octo".to_owned()),
            repo: Some("repo".to_owned()),
            ..Default::default()
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
        run_with_gateway_builder(
            &config,
            move |token, locator| {
                assert_eq!(
                    token.value(),
                    "ghp_example",
                    "unexpected token passed to gateway builder"
                );
                assert_eq!(
                    locator.api_base().as_str(),
                    "https://api.github.com/",
                    "unexpected API base for github.com locator"
                );
                Ok(gateway)
            },
            &mut buffer,
        )
        .await
        .expect("repository listing should succeed");

        let (locator, params) = captured
            .lock()
            .expect("captured mutex should be available")
            .clone()
            .expect("gateway should have been called");
        assert_eq!(locator.owner().as_str(), "octo");
        assert_eq!(locator.repository().as_str(), "repo");
        assert_eq!(params.state, Some(PullRequestState::All));
        assert_eq!(params.per_page, Some(50));
        assert_eq!(params.page, Some(1));

        let output = String::from_utf8(buffer).expect("output should be valid UTF-8");
        assert!(
            output.contains("Pull requests for octo/repo:"),
            "missing header: {output}"
        );
    }

    #[tokio::test]
    async fn run_repository_listing_propagates_invalid_pagination_error() {
        let config = FrankieConfig {
            token: Some("ghp_example".to_owned()),
            owner: Some("octo".to_owned()),
            repo: Some("repo".to_owned()),
            ..Default::default()
        };

        let gateway = CapturingGateway {
            captured: Arc::new(Mutex::new(None)),
            response: Arc::new(Mutex::new(Some(Err(IntakeError::InvalidPagination {
                message: "page must be at least 1".to_owned(),
            })))),
        };

        let mut buffer = Vec::new();
        let result =
            run_with_gateway_builder(&config, |_token, _locator| Ok(gateway), &mut buffer).await;

        assert!(
            matches!(result, Err(IntakeError::InvalidPagination { .. })),
            "expected InvalidPagination, got {result:?}"
        );
    }
}
