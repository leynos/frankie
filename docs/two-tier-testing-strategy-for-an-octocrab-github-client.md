# Two-Tier Testing Strategy for an Octocrab GitHub Client

## Overview of the Two-Tier Approach

When testing a GitHub client built on **Octocrab**, two tiers of tests are
employed for speed and confidence:

- **Unit Tests (Tier 1)** – Fast, isolated tests that **mock out Octocrab**.
  These run entirely in-memory with no network calls, using `mockall` to
  simulate Octocrab’s behaviour. Unit tests are co-located with the code
  (inside `#[cfg(test)] mod tests` in the same file) for convenient access to
  internal
  functions([1](https://github.com/microsoft/rust-for-dotnet-devs/blob/afccfb002194a51cca68d57d247d0e367cea46f2/src/testing/index.md#L9-L17)).
   This yields quick turnaround and allows testing logic without hitting the
  real GitHub API.

- **Behavioural Integration Tests (Tier 2)** – Higher-level tests that verify
  end-to-end behaviour by exercising actual HTTP calls **against a stubbed
  API**. **Wiremock** (via the Rust `wiremock` crate) runs a local HTTP server
  that mimics GitHub’s REST endpoints. Octocrab is configured to point at this
  mock server for the test, so the client code thinks it’s talking to GitHub.
  These tests run in the Rust integration test suite (files under `tests/`
  directory)([1](https://github.com/microsoft/rust-for-dotnet-devs/blob/afccfb002194a51cca68d57d247d0e367cea46f2/src/testing/index.md#L27-L32)).
   They are slower but give confidence that the client’s requests and
  Octocrab’s parsing logic work correctly against real HTTP interactions.

This two-tier strategy ensures fast feedback during development (via unit
tests) **and** thorough verification of GitHub API usage (via integration
tests). The separation (unit vs integration) **avoids hitting the real GitHub
API in tests** – instead using mocks and stubs (no real network calls) for
reliability and speed. All tests can be run with a single `cargo test`
invocation, keeping unit and BDD-style tests “on the same vine” and sharing the
same
infrastructure([2](https://github.com/leynos/rstest-bdd#:~:text=,fixtures)).

## Unit Testing with Octocrab Mocked (using *mockall*)

Unit tests validate the application logic in isolation by **mocking Octocrab’s
GitHub API calls**. The goal is to simulate responses from GitHub without
performing HTTP requests, so tests run quickly and deterministically. This is
achieved by introducing a *trait* abstraction for the GitHub client and using
`mockall` to generate a mock implementation.

### Designing a Trait for Octocrab

First, define a trait that captures the Octocrab operations the client relies
on – for example, fetching issues, listing pull request commits, getting
workflow runs, etc. Then implement this trait for the real `Octocrab` type. For
instance, if commits on a pull request must be listed alongside an issue, the
trait could be:

```rust
use octocrab::models::{Issue, RepositoryCommit};

#[cfg_attr(test, mockall::automock)] // Generates MockGitHubApi in tests
pub trait GitHubApi {
    fn get_issue(&self, owner: &str, repo: &str, number: u64)
        -> octocrab::Result<Issue>;

    fn list_pr_commits(&self, owner: &str, repo: &str, pr_number: u64)
        -> octocrab::Result<Vec<RepositoryCommit>>;

    // ... other methods as needed, e.g., list_action_runs, etc.
}

// Real Octocrab implements the trait
impl GitHubApi for octocrab::Octocrab {
    fn get_issue(&self, owner: &str, repo: &str, number: u64)
            -> octocrab::Result<Issue> {
        self.issues(owner, repo).get(number).await
    }

    fn list_pr_commits(&self, owner: &str, repo: &str, pr: u64)
            -> octocrab::Result<Vec<RepositoryCommit>> {
        // Octocrab returns Page<RepositoryCommit>, but for simplicity:
        self.pulls(owner, repo).list_commits(pr).await
    }
}
```

Here, `#[cfg_attr(test, automock)]` ensures that in test builds, `mockall`
creates a `MockGitHubApi` struct
automatically([3](https://medium.com/@cuongleqq/unlock-100-coverage-mock-your-rust-unit-tests-the-right-way-3afbabc5dc5e#:~:text=,Result%3CTxid%3E%3B)).
 Production code can use `Octocrab` (which implements `GitHubApi`) normally,
while tests can substitute the mock.

> **Why a trait?** In Rust, using a trait as an abstraction layer enables
> swapping the real GitHub client for a mock one in unit
> tests([3](https://medium.com/@cuongleqq/unlock-100-coverage-mock-your-rust-unit-tests-the-right-way-3afbabc5dc5e#:~:text=,integration%20tests%20to%20catch%20surprises)).
> This follows the dependency-inversion principle and keeps the code testable.

Ensure the application code depends on `GitHubApi` (e.g. pass a
`&dyn GitHubApi` into functions or have structs hold a `Box<dyn GitHubApi>` or
generic parameter). This enables unit tests to inject a `MockGitHubApi`.

### Writing Unit Tests with `MockGitHubApi`

Unit tests reside alongside code in the same
module([1](https://github.com/microsoft/rust-for-dotnet-devs/blob/afccfb002194a51cca68d57d247d0e367cea46f2/src/testing/index.md#L9-L17)).
 The `MockGitHubApi` simulates various API responses and verifies the logic.
Here's an example unit test using the mock:

```rust
use mockall::predicate::*;

#[tokio::test]
async fn test_process_pull_request() {
    let owner = "myorg";
    let repo = "myrepo";
    let pr_number = 42;

    // Set up mock Octocrab (GitHubApi)
    let mut mock_api = MockGitHubApi::new();

    // Prepare a dummy commit list to return
    let fake_commits = vec![RepositoryCommit { /* ...fields... */ }];
    // Expect that list_pr_commits will be called with specified parameters
    mock_api
        .expect_list_pr_commits()
        .with(eq(owner), eq(repo), eq(pr_number))
        .returning(move |_, _, _| Ok(fake_commits.clone()));

    // Prepare a dummy issue
    let issue_number = 101;
    let fake_issue = Issue { /* ...fields... */ };
    mock_api
        .expect_get_issue()
        .with(eq(owner), eq(repo), eq(issue_number))
        .return_once(move |_, _, _| Ok(fake_issue));

    // Call the function under test, injecting the mock API
    let result = my_code::process_pull_and_issue(&mock_api, pr_number, issue_number).await;

    // Verify the result uses data from the fake commits/issue correctly
    assert!(result.is_ok());
    // ...more assertions...
}
```

In this example, the test sets expectations on the mock: it expects
`list_pr_commits(owner, repo, 42)` to be called, and defines it to return a
predefined vector of `RepositoryCommit` objects. Similarly, `get_issue` is
expected with a specific issue number, returning a fake `Issue`.
`process_pull_and_issue` (which internally calls `api.list_pr_commits` and
`api.get_issue`) receives these fake responses. Assertions then confirm that
`process_pull_and_issue` produced the correct output based on the stubbed data.
This pattern allows unit tests to cover logic like filtering, transformations,
and error handling by forcing various scenarios (e.g., the mock can return an
error to exercise error paths).

**Advantages:** Using mocks for unit tests keeps them **fast and reliable**.
There are no HTTP calls or large data parsing – the tests run synchronously (or
with a local `tokio` runtime as above) and complete in milliseconds. Edge cases
(like Octocrab returning errors or empty data) can also be simulated easily. As
a best practice, **use mocks for external calls in unit tests** to avoid
slowness or
flakiness([3](https://medium.com/@cuongleqq/unlock-100-coverage-mock-your-rust-unit-tests-the-right-way-3afbabc5dc5e#:~:text=You%20might%20be%20wondering%3A%20Should,the%20pain%2C%20not%20the%20world)).

**Test placement:** Keep these fast unit tests in the module they test. Rust
will compile them with the library, but only run them when `cargo test` is
executed. Co-location also provides access to private functions without
exposing them via public
API([1](https://github.com/microsoft/rust-for-dotnet-devs/blob/afccfb002194a51cca68d57d247d0e367cea46f2/src/testing/index.md#L14-L22)).

## Integration Tests with Wiremock (Behavioural Tests)

For higher-level confidence, the test suite includes **integration tests** that
exercise the real HTTP request/response cycle of the GitHub client. The
`wiremock` crate stands up a local server that mimics GitHub’s API. The
Octocrab-based client sends real HTTP requests to this server, and assertions
verify it handles the responses correctly. This approach is inspired by
Octocrab’s own test suite, which uses Wiremock to simulate GitHub
endpoints([4](https://github.com/LukeMathWalker/wiremock-rs#:~:text=%2F%2F%20Start%20a%20background%20HTTP,await)).

### Setting Up a Wiremock Server in Tests

Each integration test (or test scenario) will create its own **`MockServer`**
instance. Wiremock makes this easy:

```rust
use wiremock::{MockServer, Mock, ResponseTemplate};
use wiremock::matchers::{method, path};

#[tokio::test]
async fn test_list_pull_request_commits() -> octocrab::Result<()> {
    // Start a background HTTP server on a random local port:contentReference[oaicite:9]{index=9}
    let mock_server = MockServer::start().await;

    // Arrange: stub the GitHub API endpoint for listing PR commits
    let owner = "myorg";
    let repo = "myrepo";
    let pr_number = 42;
    let api_path = format!("/repos/{}/{}/pulls/{}/commits", owner, repo, pr_number);
    // Load a sample API response body (JSON) for commits
    let sample_body = include_str!("fixtures/pull_commits_42.json");
    Mock::given(method("GET"))
        .and(path(api_path))
        .respond_with(ResponseTemplate::new(200)
            .set_body_raw(sample_body, "application/json"))
        .mount(&mock_server)
        .await;
    // ^ This tells the mock server: when a GET request hits the specified path, respond with HTTP 200 and the JSON body:contentReference[oaicite:10]{index=10}.

    // Configure Octocrab to use the mock server's base URL instead of api.github.com
    let octo = octocrab::OctocrabBuilder::new()
        .personal_token("dummy-token")              // (use a dummy token if needed)
        .base_uri(mock_server.uri())?               // point to the mock GitHub server:contentReference[oaicite:11]{index=11}
        .build()?;

    // Act: call the Octocrab client to list PR commits (this will hit the mock server)
    let page = octo.pulls(owner, repo).list_commits(pr_number).await?;
    let commits = page.items;  // assuming Octocrab returns a Page<RepositoryCommit>

    // Assert: verify the response was parsed as expected
    assert!(!commits.is_empty(), "Expected commits in the response");
    assert_eq!(commits[0].sha, "abc1234...", "First commit SHA should match fixture");
    // ... additional asserts on commits ...

    // (Wiremock will automatically verify that the expected call was made; any unmatched calls result in a 404:contentReference[oaicite:12]{index=12}.)
    Ok(())
}
`
```

This integration test simulates a **“List PR Commits”** API call:

- A `MockServer` starts (on a random port to avoid conflicts).

- `Mock::given(...)` with matchers stubs the exact HTTP request path and method
  the client should call, and `respond_with(...)` provides a canned JSON
  response([4](https://github.com/LukeMathWalker/wiremock-rs#:~:text=%2F%2F%20when%20it%20receives%20a,await)).
   In this case, the stubbed endpoint is
  `GET /repos/myorg/myrepo/pulls/42/commits`.

- Include a sample JSON payload (perhaps stored in a `fixtures/` file for
  realism) that represents what GitHub’s API would return for that request.

- Build an `Octocrab` instance with its base URL set to the mock server’s URI
  (e.g. `http://127.0.0.1:XXXXX` where the server is listening). Octocrab’s
  builder supports overriding the base API
  URL([5](https://docs.rs/octocrab/latest/octocrab/struct.OctocrabBuilder.html#:~:text=,112%3CSelf)),
   which is intended for GitHub Enterprise or testing. This ensures all
  requests from this `octo` client go to the `MockServer` instead of the real
  `api.github.com` domain.

- When the test calls `octo.pulls(owner, repo).list_commits(pr_number).await`,
  Octocrab will generate an HTTP request to
  `http://127.0.0.1:PORT/repos/myorg/myrepo/pulls/42/commits`. The Wiremock
  server intercepts this, finds the matching stub and returns the predefined
  JSON with a 200 OK. Octocrab then parses that JSON into `RepositoryCommit`
  objects exactly as it would with real data.

- Finally, assertions confirm that the returned data matches expectations (for
  example, checking that at least one commit was returned and perhaps verifying
  specific fields against what was placed in the fixture JSON).

This kind of test is slower than a pure unit test (it involves JSON
serialization and an HTTP round-trip locally), but it **validates the
integration of several pieces**: the code’s usage of Octocrab, Octocrab’s
request formation, and its model parsing. It’s a true behaviour test of the
system.

**Test organization:** These are placed in `tests/` as integration tests, each
file compiled as a separate crate by
Cargo([1](https://github.com/microsoft/rust-for-dotnet-devs/blob/afccfb002194a51cca68d57d247d0e367cea46f2/src/testing/index.md#L27-L32)).
 Async tests with `tokio::test` (or `rstest` with async support) still work for
this suite.

**Tip:** **Do not reuse a single `MockServer` across tests.** Each test (or
each scenario) should spawn its own `MockServer` on a fresh port for
isolation([4](https://github.com/LukeMathWalker/wiremock-rs#:~:text=Each%20instance%20of%20MockServer%20is,assigned%20to%20the%20new%20MockServer)).
 This prevents interference between tests and allows running tests in parallel
safely. The Wiremock server will automatically shut down when it goes out of
scope at test end, freeing the
port([4](https://github.com/LukeMathWalker/wiremock-rs#:~:text=MockServers%20should%20be%20created%20in,test%20where%20they%20are%20used)).

### Using `rstest` Fixtures and `rstest-bdd` for Clarity

This guide integrates **`rstest`** and **`rstest-bdd`** to improve test
maintainability:

- **Fixtures:** With `rstest::fixture`, common setup such as starting the mock
  server or constructing the Octocrab client can be factored out. For example,
  a fixture could start a `MockServer`, register common stubs (or none, letting
  each test add its own), and return an `Octocrab` instance pointed at that
  server. This avoids repeating setup code in every test. Fixtures can also
  load JSON from disk or create common data structures (like a template Issue
  or PR object) for reuse.

```rust
use rstest::fixture;
use wiremock::MockServer;

#[fixture]
async fn github_server() -> MockServer {
    MockServer::start().await
}

#[fixture]
async fn octocrab_client(github_server: MockServer) -> octocrab::Octocrab {
    // Optionally set up default mocks here or leave it to each test
    octocrab::OctocrabBuilder::new()
        .personal_token("test-token")
        .base_uri(github_server.uri())
        .expect("valid base URL") // :contentReference[oaicite:18]{index=18}
        .build()
        .expect("Octocrab build")
}
```

Each test can accept `octocrab_client` (and `github_server` if needed) as
arguments, and `rstest` will provide the fixture values. Because fixtures are
first-class, they can be shared between unit, integration, and BDD tests
seamlessly([2](https://github.com/leynos/rstest-bdd#:~:text=%60rstest,fixture%20and%20parametrisation%20model)).

- **Behaviour-Driven Scenarios:** The `rstest-bdd` crate allows writing tests in
  a Gherkin-style Given/When/Then format using attributes. Human-readable
  scenarios can correspond to integration tests. For example, a feature file
  might be used, or the macros can be applied directly:

```rust
use rstest_bdd::{given, when, then, scenario};

#[given("a repository has an open pull request with commits")]
async fn a_repo_with_pr(github_server: &MockServer) {
    // Stub the GET /repos/:owner/:repo/pulls/:pr/commits endpoint for this scenario
    let body = include_str!("fixtures/pr_commits_example.json");
    Mock::given(method("GET"))
        .and(path("/repos/myorg/myrepo/pulls/42/commits"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(body, "application/json"))
        .mount(github_server)
        .await;
}

#[when("I request the list of commits on that pull request")]
async fn request_pr_commits(octocrab_client: &octocrab::Octocrab) -> octocrab::Result<Vec<RepositoryCommit>> {
    let page = octocrab_client.pulls("myorg", "myrepo").list_commits(42).await?;
    Ok(page.items)
}

#[then("I should get back the commits in the response")]
fn should_get_commits(commits: &Vec<RepositoryCommit>) {
    assert!(!commits.is_empty());
    // further assertions...
}

#[scenario]
#[tokio::test]
async fn list_pull_request_commits(
    #[future] github_server: MockServer,
    #[future] octocrab_client: octocrab::Octocrab,
) {
    // This test will automatically run the given, when, then in order.
}
```

In the above (illustrative) scenario, the context is described in the
`#[given]` step (setting up the stubbed API), the action in the `#[when]` step
(calling the client), and the outcome in the `#[then]` step (asserting on the
result). The `#[scenario]` test ties them together. Under the hood, this uses
the fixtures `github_server` and `octocrab_client` (marked with `#[future]` to
indicate they're async fixtures) and executes the steps. This BDD style can
make complex integration tests more readable and is fully powered by `rstest`
(so it runs with `cargo test` like any other
test([2](https://github.com/leynos/rstest-bdd#:~:text=,fixtures))). It’s mainly
a matter of preference – equivalent tests can be written without BDD syntax –
but it can be very effective for communication.

*Note:* Each scenario corresponds to one logical test case (multiple scenarios
per feature are fine). Also, `rstest-bdd` allows using `.feature` files if
desired, but scenarios can be embedded in code as shown.

### Handling Actions Runs and Issues in Tests

The pattern demonstrated for pull request commits applies similarly to other
GitHub data:

- **Issues:** For example, testing an issue retrieval or commenting flow can be
  done by stubbing `GET /repos/:owner/:repo/issues/:number` or
  `POST /repos/:owner/:repo/issues/:number/comments`, etc., with appropriate
  JSON responses. An integration test would configure the stub via
  `Mock::given(method("GET")).and(path("/repos/ORG/REPO/issues/123"))...respond_with(...)`,
   then call `octocrab_client.issues("ORG","REPO").get(123).await` and assert
  on the returned `Issue`. Creating a comment can also be simulated by stubbing
  the POST and verifying (Wiremock supports verifying that a request was
  received a certain number of times or with certain body
  content([4](https://github.com/LukeMathWalker/wiremock-rs#:~:text=,expect%20method%20for%20more%20details))).

- **Actions Workflow Runs:** Octocrab supports listing workflow runs via
  `octocrab.workflows(owner, repo).list_runs(...)`. To test this, stub the
  appropriate endpoint (e.g. `GET /repos/:owner/:repo/actions/runs` or the
  specific workflow runs path). Provide sample JSON for a list of workflow runs
  (matching GitHub’s response schema). Then call
  `octocrab_client.workflows("owner","repo").list_runs(...).await` in the test
  and assert that the returned list of `Run` objects matches the fixture.
  Filters (query parameters) can be included in the stubbed path if the code
  uses them (Wiremock’s `path` matcher can include query strings or the
  `wiremock::matchers::query_param` matcher can be used for more precision).

In all cases, the **arrange-act-assert** flow remains: use Wiremock to
**arrange** a known API response for a given request, **act** by invoking the
Octocrab client, then **assert** that the result matches expectations.

## Effective Use of Fixtures and Configuration

**Fixture Data:** Keep example JSON responses in files under a `fixtures/`
directory (in the test folder). This keeps test code cleaner and allows reusing
realistic payloads. For instance, `pull_commits_42.json` could be an actual
truncated JSON from GitHub’s API for list commits. By using `include_str!` or
reading the file at runtime, the payload can be fed into
`ResponseTemplate::set_body_raw`. This ensures that each test validates parsing
against real-world data shapes.

**Octocrab Configuration:** Other than `base_uri`, configure Octocrab with a
**dummy authentication** for consistency. For example, use
`.personal_token("test-token")` or `.app(...)` if the code typically uses app
authentication – Octocrab doesn’t actually verify the token in client-side
code, and Wiremock will accept any headers unless specifically stubbed. The
idea is to make the test client as close to the real configuration as possible
(so that any auth or header logic in Octocrab is exercised), while pointing to
a safe endpoint. Octocrab’s builder also lets you set timeouts, add previews,
etc., if the code requires those; configure them in the test client as needed
to mirror production settings.

**Sharing State:** Each test should ideally be self-contained. If common
behaviour across many tests is required (e.g., every test needs the
`/repos/:owner/:repo` base path stubbed in some way), use a *background* in BDD
or a fixture that mounts certain default stubs. Be cautious: tests running in
parallel should not share a server or mutable state without synchronization.
Using fixtures that spawn fresh servers per test and mount only the stubs
needed for that specific scenario is usually the safest route (as shown above).

Finally, remember that **unit tests and integration tests complement each
other**. Unit tests using mocks will cover logic quickly (e.g., how the code
reacts to various Octocrab results, including error cases), while integration
tests with Wiremock ensure usage of Octocrab actually aligns with the real
GitHub API contracts. This two-tier approach, combined with `rstest` fixtures
and BDD-style clarity, yields a robust test suite:

- **Fast feedback:** If something fails in business logic, a unit test likely
  catches it within seconds.

- **End-to-end confidence:** If the GitHub integration breaks (say due to a
  changed endpoint or a bug in request formatting), a Wiremock-backed test will
  flag it.

- **Maintainability:** Clear, behaviour-focused tests (especially using
  Given/When/Then descriptions) serve as documentation for how the GitHub
  client is supposed to work.

By following this strategy, the Octocrab-based application is thoroughly tested
at both the small-scale and large-scale. Pain points are mocked for speed, and
the real world is simulated for accuracy – all within the comfort of
`cargo test` (no external dependencies or actual API calls needed).

**Sources:**

- Rust testing conventions for unit vs integration
  tests([1](https://github.com/microsoft/rust-for-dotnet-devs/blob/afccfb002194a51cca68d57d247d0e367cea46f2/src/testing/index.md#L9-L17))([1](https://github.com/microsoft/rust-for-dotnet-devs/blob/afccfb002194a51cca68d57d247d0e367cea46f2/src/testing/index.md#L27-L32))

- Using `mockall::automock` to generate mocks for unit
  tests([3](https://medium.com/@cuongleqq/unlock-100-coverage-mock-your-rust-unit-tests-the-right-way-3afbabc5dc5e#:~:text=,Result%3CTxid%3E%3B))

- Guidance on when to use mocks vs real calls in
  tests([3](https://medium.com/@cuongleqq/unlock-100-coverage-mock-your-rust-unit-tests-the-right-way-3afbabc5dc5e#:~:text=You%20might%20be%20wondering%3A%20Should,the%20pain%2C%20not%20the%20world))

- Example of setting up a Wiremock server and stubbing an
  endpoint([4](https://github.com/LukeMathWalker/wiremock-rs#:~:text=%2F%2F%20Start%20a%20background%20HTTP,await))

- Wiremock documentation on test isolation (one server per
  test)([4](https://github.com/LukeMathWalker/wiremock-rs#:~:text=Each%20instance%20of%20MockServer%20is,assigned%20to%20the%20new%20MockServer))

- Octocrab’s builder allowing custom base URL
  configuration([5](https://docs.rs/octocrab/latest/octocrab/struct.OctocrabBuilder.html#:~:text=let%20octocrab%20%3D%20octocrab%3A%3AOctocrabBuilder%3A%3Adefault%28%29%20.add_preview%28%22machine,.build))

- rstest-bdd documentation (combining unit and acceptance tests with shared
  fixtures)([2](https://github.com/leynos/rstest-bdd#:~:text=,fixtures))([2](https://github.com/leynos/rstest-bdd#:~:text=%60rstest,fixture%20and%20parametrisation%20model))
