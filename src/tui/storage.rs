//! Startup context storage and refresh helpers for the review TUI.
//!
//! This module owns the global `OnceLock` values used during TUI bootstrapping
//! and provides the setter/getter functions consumed by CLI wiring and app
//! handlers.

use std::sync::{Arc, OnceLock};

use crossterm::terminal;

use crate::github::error::IntakeError;
use crate::github::locator::{PersonalAccessToken, PullRequestLocator};
use crate::github::models::ReviewComment;
use crate::local::GitOperations;
use crate::telemetry::{NoopTelemetrySink, TelemetryEvent, TelemetrySink};

/// Global storage for initial review data.
///
/// This is set before the TUI program starts and read by `ReviewApp::init()`.
static INITIAL_REVIEWS: OnceLock<Vec<ReviewComment>> = OnceLock::new();

/// Global storage for initial terminal dimensions.
///
/// This is set before the TUI program starts and read by `ReviewApp::new()`
/// so the first frame uses the actual terminal size.
static INITIAL_TERMINAL_SIZE: OnceLock<(u16, u16)> = OnceLock::new();

/// Global storage for refresh context (locator and token).
///
/// This is set before the TUI program starts to enable refresh functionality.
static REFRESH_CONTEXT: OnceLock<RefreshContext> = OnceLock::new();

/// Global storage for telemetry sink.
///
/// This is set before the TUI program starts to enable sync latency metrics.
static TELEMETRY_SINK: OnceLock<Arc<dyn TelemetrySink>> = OnceLock::new();

/// Static fallback telemetry sink to avoid allocations on each call.
///
/// This is used by `get_telemetry_sink` when no sink has been configured,
/// avoiding repeated `Arc::new` allocations.
static DEFAULT_TELEMETRY_SINK: OnceLock<Arc<dyn TelemetrySink>> = OnceLock::new();

/// Global storage for Git operations context.
///
/// This is set before the TUI program starts when a valid local repository
/// is discovered or configured. Enables time-travel navigation in the TUI.
static GIT_OPS_CONTEXT: OnceLock<GitOpsContext> = OnceLock::new();

/// Global storage for time-travel context (PR info and discovery status).
///
/// Always set before TUI startup; provides context for error messages when
/// time-travel is attempted without a valid local repository.
static TIME_TRAVEL_CONTEXT: OnceLock<TimeTravelContext> = OnceLock::new();

/// Context required to refresh review data from GitHub.
struct RefreshContext {
    locator: PullRequestLocator,
    token: PersonalAccessToken,
}

/// Git operations context for time-travel navigation.
struct GitOpsContext {
    git_ops: Arc<dyn GitOperations>,
    head_sha: String,
}

/// Context describing the PR and any discovery failure for error messages.
///
/// Stored alongside git ops to provide contextual error messages when
/// time-travel is attempted without a valid local repository.
#[derive(Debug, Clone, PartialEq)]
pub struct TimeTravelContext {
    /// PR host (e.g. "github.com" or "ghe.corp.com").
    pub host: String,
    /// PR owner (e.g. "octocat").
    pub owner: String,
    /// PR repository name (e.g. "hello-world").
    pub repo: String,
    /// PR number.
    pub pr_number: u64,
    /// Reason discovery failed, if applicable.
    pub discovery_failure: Option<String>,
}

/// Sets the initial reviews for the TUI application.
///
/// This must be called before starting the bubbletea-rs program. The reviews
/// will be read by `ReviewApp::init()` when the program starts.
///
/// # Arguments
///
/// * `reviews` - The review comments to display initially.
///
/// # Returns
///
/// `true` if the reviews were set, `false` if they were already set.
pub fn set_initial_reviews(reviews: Vec<ReviewComment>) -> bool {
    INITIAL_REVIEWS.set(reviews).is_ok()
}

/// Sets the initial terminal dimensions for the TUI application.
///
/// This should be called before starting the bubbletea-rs program so the
/// initial render can use the actual terminal size instead of fallbacks.
///
/// # Arguments
///
/// * `width` - Terminal width in columns.
/// * `height` - Terminal height in rows.
///
/// # Returns
///
/// `true` if the dimensions were set, `false` if they were already set.
pub fn set_initial_terminal_size(width: u16, height: u16) -> bool {
    INITIAL_TERMINAL_SIZE.set((width, height)).is_ok()
}

/// Sets the refresh context for the TUI application.
///
/// This must be called before starting the bubbletea-rs program to enable
/// the refresh feature. Without this context, refresh requests will fail
/// with an error message.
///
/// # Arguments
///
/// * `locator` - The pull request locator for API calls.
/// * `token` - The personal access token for authentication.
///
/// # Returns
///
/// `true` if the context was set, `false` if it was already set.
pub fn set_refresh_context(locator: PullRequestLocator, token: PersonalAccessToken) -> bool {
    REFRESH_CONTEXT
        .set(RefreshContext { locator, token })
        .is_ok()
}

/// Sets the telemetry sink for the TUI application.
///
/// This must be called before starting the bubbletea-rs program to enable
/// sync latency metrics. Without this, a no-op sink is used.
///
/// # Arguments
///
/// * `sink` - The telemetry sink to use for recording events.
///
/// # Returns
///
/// `true` if the sink was set, `false` if it was already set.
pub fn set_telemetry_sink(sink: Arc<dyn TelemetrySink>) -> bool {
    TELEMETRY_SINK.set(sink).is_ok()
}

/// Sets the Git operations context for time-travel navigation.
///
/// This must be called before starting the bubbletea-rs program. When a
/// valid local repository is available, this enables time-travel features.
///
/// # Arguments
///
/// * `git_ops` - The Git operations implementation.
/// * `head_sha` - The HEAD commit SHA for line mapping verification.
///
/// # Returns
///
/// `true` if the context was set, `false` if it was already set.
pub fn set_git_ops_context(git_ops: Arc<dyn GitOperations>, head_sha: String) -> bool {
    GIT_OPS_CONTEXT
        .set(GitOpsContext { git_ops, head_sha })
        .is_ok()
}

/// Sets the time-travel context (PR info and discovery status).
///
/// This must be called before starting the bubbletea-rs program. It stores
/// PR metadata used to generate contextual error messages when time-travel
/// is unavailable.
///
/// # Returns
///
/// `true` if the context was set, `false` if it was already set.
pub fn set_time_travel_context(context: TimeTravelContext) -> bool {
    TIME_TRAVEL_CONTEXT.set(context).is_ok()
}

/// Gets the Git operations context, if configured.
///
/// Called internally by `ReviewApp::init()`. Returns the stored git ops
/// and HEAD SHA, or `None` if no local repository was configured.
pub(crate) fn get_git_ops_context() -> Option<(Arc<dyn GitOperations>, String)> {
    GIT_OPS_CONTEXT
        .get()
        .map(|ctx| (Arc::clone(&ctx.git_ops), ctx.head_sha.clone()))
}

/// Gets the time-travel context, if configured.
///
/// Called internally by the time-travel error handler to generate
/// contextual error messages.
pub(crate) fn get_time_travel_context() -> Option<TimeTravelContext> {
    TIME_TRAVEL_CONTEXT.get().cloned()
}

/// Gets the telemetry sink, returning a no-op sink if not configured.
///
/// Uses a static fallback sink to avoid allocating a new `Arc` on each call
/// when no sink has been configured.
fn get_telemetry_sink() -> Arc<dyn TelemetrySink> {
    TELEMETRY_SINK.get().cloned().unwrap_or_else(|| {
        Arc::clone(DEFAULT_TELEMETRY_SINK.get_or_init(|| Arc::new(NoopTelemetrySink)))
    })
}

/// Records sync telemetry for a completed sync operation.
///
/// Called internally by the app after a successful sync.
pub(crate) fn record_sync_telemetry(latency_ms: u64, comment_count: usize, incremental: bool) {
    get_telemetry_sink().record(TelemetryEvent::SyncLatencyRecorded {
        latency_ms,
        comment_count,
        incremental,
    });
}

/// Gets a clone of the initial reviews from storage.
///
/// Called internally by `ReviewApp::init()`. Returns the stored reviews or
/// an empty vector if not set.
///
/// Note: This function clones the data because `OnceLock` does not support
/// consuming (taking) the value. The name reflects that this is a read
/// operation, not a destructive take.
pub(crate) fn get_initial_reviews() -> Vec<ReviewComment> {
    INITIAL_REVIEWS.get().cloned().unwrap_or_default()
}

/// Gets the initial terminal dimensions from storage.
///
/// Called internally by `ReviewApp::new()`. Returns the stored dimensions or
/// fallback dimensions if none were set.
pub(crate) fn get_initial_terminal_size() -> (u16, u16) {
    const DEFAULT_WIDTH: u16 = 80;
    const DEFAULT_HEIGHT: u16 = 24;

    INITIAL_TERMINAL_SIZE
        .get()
        .copied()
        .filter(|(width, height)| *width > 0 && *height > 0)
        .or_else(|| {
            terminal::size()
                .ok()
                .filter(|(width, height)| *width > 0 && *height > 0)
        })
        .unwrap_or((DEFAULT_WIDTH, DEFAULT_HEIGHT))
}

/// Returns the configured pull request locator for refresh-dependent features.
#[must_use]
pub(crate) fn get_refresh_locator() -> Option<PullRequestLocator> {
    REFRESH_CONTEXT.get().map(|context| context.locator.clone())
}

/// Fetches fresh review comments from GitHub.
///
/// Uses the refresh context set by [`set_refresh_context`]. Returns an error
/// if the context was not set or if the API call fails.
pub(crate) async fn fetch_reviews() -> Result<Vec<ReviewComment>, IntakeError> {
    use crate::github::gateway::{OctocrabReviewCommentGateway, ReviewCommentGateway};

    let context = REFRESH_CONTEXT.get().ok_or_else(|| IntakeError::Api {
        message: "Refresh context not configured".to_owned(),
    })?;

    let gateway =
        OctocrabReviewCommentGateway::new(&context.token, context.locator.api_base().as_str())?;
    gateway.list_review_comments(&context.locator).await
}
