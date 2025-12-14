//! Scenario state and runtime/server initialisation for the repository listing
//! BDD tests.

use frankie::{
    IntakeError, ListPullRequestsParams, OctocrabRepositoryGateway, PaginatedPullRequests,
    PersonalAccessToken, RepositoryIntake, RepositoryLocator,
};
use rstest_bdd::Slot;
use rstest_bdd_macros::ScenarioState;
use std::cell::RefCell;
use std::rc::Rc;
use tokio::runtime::Runtime;
use wiremock::MockServer;

use super::domain::PageNumber;

/// Shared runtime wrapper that can be stored in rstest-bdd Slot.
#[derive(Clone)]
pub(crate) struct SharedRuntime(Rc<RefCell<Runtime>>);

impl SharedRuntime {
    pub(crate) fn new(runtime: Runtime) -> Self {
        Self(Rc::new(RefCell::new(runtime)))
    }

    pub(crate) fn block_on<F: std::future::Future>(&self, future: F) -> F::Output {
        self.0.borrow().block_on(future)
    }
}

#[derive(ScenarioState, Default)]
pub(crate) struct ListingState {
    pub(crate) runtime: Slot<SharedRuntime>,
    pub(crate) server: Slot<MockServer>,
    pub(crate) token: Slot<String>,
    pub(crate) page: Slot<u32>,
    pub(crate) result: Slot<PaginatedPullRequests>,
    pub(crate) error: Slot<IntakeError>,
}

/// Ensures the runtime and server are initialised in `ListingState`.
pub(crate) fn ensure_runtime_and_server(listing_state: &ListingState) -> SharedRuntime {
    if listing_state.runtime.with_ref(|_| ()).is_none() {
        let runtime = Runtime::new()
            .unwrap_or_else(|error| panic!("failed to create Tokio runtime: {error}"));
        listing_state.runtime.set(SharedRuntime::new(runtime));
    }

    let shared_runtime = listing_state
        .runtime
        .get()
        .unwrap_or_else(|| panic!("runtime not initialised after set"));

    if listing_state.server.with_ref(|_| ()).is_none() {
        listing_state
            .server
            .set(shared_runtime.block_on(MockServer::start()));
    }

    shared_runtime
}

pub(crate) fn run_repository_listing(
    listing_state: &ListingState,
    repo_url: &str,
    page: PageNumber,
) -> Result<PaginatedPullRequests, IntakeError> {
    let server_url = listing_state
        .server
        .with_ref(MockServer::uri)
        .ok_or_else(|| IntakeError::Api {
            message: "mock server URL missing".to_owned(),
        })?;

    let resolved_url = resolve_mock_server_url(&server_url, repo_url);
    let locator = RepositoryLocator::parse(&resolved_url)?;

    listing_state.page.set(page.value());

    let runtime = get_runtime(listing_state)?;

    runtime.block_on(async {
        let token_value = listing_state.token.get().ok_or(IntakeError::MissingToken)?;
        let token = PersonalAccessToken::new(token_value)?;

        let gateway = OctocrabRepositoryGateway::for_token(&token, &locator)?;
        let intake = RepositoryIntake::new(&gateway);
        let params = ListPullRequestsParams {
            page: Some(page.value()),
            per_page: Some(50),
            ..Default::default()
        };

        intake.list_pull_requests(&locator, &params).await
    })
}

fn resolve_mock_server_url(server_url: &str, repo_url: &str) -> String {
    let cleaned_url = repo_url.trim_matches('"');
    if cleaned_url.contains("://SERVER") {
        cleaned_url
            .replace("https://SERVER", server_url)
            .replace("http://SERVER", server_url)
    } else {
        cleaned_url.replace("SERVER", server_url)
    }
}

fn get_runtime(listing_state: &ListingState) -> Result<SharedRuntime, IntakeError> {
    listing_state.runtime.get().ok_or_else(|| IntakeError::Api {
        message: "runtime not initialised".to_owned(),
    })
}
