//! Terminal User Interface for review listing and filtering.
//!
//! Provides an interactive TUI for navigating and filtering pull request
//! review comments using the bubbletea-rs Model-View-Update (MVU) pattern.
//!
//! Because bubbletea-rs requires `Model::init()` to be static, this module
//! uses `OnceLock`-based storage for initial data, refresh context, git
//! operations, time-travel context, and telemetry. Call the corresponding
//! `set_*` functions before starting the program.

use std::sync::{Arc, OnceLock};

use crate::github::error::IntakeError;
use crate::github::locator::{PersonalAccessToken, PullRequestLocator};
use crate::github::models::ReviewComment;
use crate::local::GitOperations;
use crate::telemetry::{NoopTelemetrySink, TelemetryEvent, TelemetrySink};
use crossterm::terminal;

pub mod app;
pub mod components;
pub mod input;
pub mod messages;
pub mod state;
pub mod sync;

pub use app::ReviewApp;

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

/// Global storage for reply-drafting configuration.
///
/// This is set before TUI startup from CLI/config sources. When not provided,
/// the application falls back to built-in defaults.
static REPLY_DRAFT_CONFIG: OnceLock<ReplyDraftConfig> = OnceLock::new();

/// Static fallback reply-drafting configuration.
static DEFAULT_REPLY_DRAFT_CONFIG: OnceLock<ReplyDraftConfig> = OnceLock::new();

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

/// Configuration for template-based reply drafting inside the TUI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplyDraftConfig {
    /// Maximum character count for reply drafts.
    pub max_length: usize,
    /// Ordered template list mapped to keyboard slots `1`-`9`.
    pub templates: Vec<String>,
}

impl Default for ReplyDraftConfig {
    fn default() -> Self {
        Self {
            max_length: 500,
            templates: vec![
                "Thanks for the review on {{ file }}:{{ line }}. I will update this.".to_owned(),
                "Good catch, {{ reviewer }}. I will address this in the next commit.".to_owned(),
                "I have addressed this feedback and pushed an update.".to_owned(),
            ],
        }
    }
}

impl ReplyDraftConfig {
    /// Creates a reply-drafting config while normalising invalid lengths.
    #[must_use]
    pub fn new(max_length: usize, templates: Vec<String>) -> Self {
        Self {
            max_length: max_length.max(1),
            templates,
        }
    }
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

/// Sets reply-drafting configuration for TUI startup.
///
/// Returns `true` when the value is set for the first time, or `false` when a
/// prior value already exists.
pub fn set_reply_draft_config(config: ReplyDraftConfig) -> bool {
    REPLY_DRAFT_CONFIG
        .set(ReplyDraftConfig::new(config.max_length, config.templates))
        .is_ok()
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

/// Gets reply-drafting configuration, falling back to defaults.
pub(crate) fn get_reply_draft_config() -> ReplyDraftConfig {
    REPLY_DRAFT_CONFIG.get().cloned().unwrap_or_else(|| {
        DEFAULT_REPLY_DRAFT_CONFIG
            .get_or_init(ReplyDraftConfig::default)
            .clone()
    })
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

#[cfg(test)]
#[expect(
    clippy::ref_option_ref,
    reason = "Generated by mockall macro for Option<&T> parameters"
)]
mod tests {
    use std::sync::Arc;

    use mockall::mock;

    use crate::local::{
        CommitSha, CommitSnapshot, GitOperationError, GitOperations, LineMappingRequest,
        LineMappingVerification, RepoFilePath,
    };
    use crate::telemetry::test_support::RecordingTelemetrySink;
    use crate::telemetry::{NoopTelemetrySink, TelemetryEvent, TelemetrySink};

    use super::*;

    mock! {
        pub GitOps {}

        impl std::fmt::Debug for GitOps {
            fn fmt<'a>(&self, f: &mut std::fmt::Formatter<'a>) -> std::fmt::Result;
        }

        impl GitOperations for GitOps {
            fn get_commit_snapshot<'a>(
                &self,
                sha: &'a CommitSha,
                file_path: Option<&'a RepoFilePath>,
            ) -> Result<CommitSnapshot, GitOperationError>;

            fn get_file_at_commit<'a>(
                &self,
                sha: &'a CommitSha,
                file_path: &'a RepoFilePath,
            ) -> Result<String, GitOperationError>;

            fn verify_line_mapping<'a>(
                &self,
                request: &'a LineMappingRequest,
            ) -> Result<LineMappingVerification, GitOperationError>;

            fn get_parent_commits<'a>(
                &self,
                sha: &'a CommitSha,
                limit: usize,
            ) -> Result<Vec<CommitSha>, GitOperationError>;

            fn commit_exists<'a>(&self, sha: &'a CommitSha) -> bool;
        }
    }

    #[test]
    fn get_telemetry_sink_returns_usable_sink() {
        // OnceLock may return Noop or a previously-set sink; verify no panic.
        let sink = get_telemetry_sink();
        sink.record(TelemetryEvent::SyncLatencyRecorded {
            latency_ms: 100,
            comment_count: 5,
            incremental: true,
        });
    }

    #[test]
    fn noop_telemetry_sink_can_record_without_panic() {
        let sink = NoopTelemetrySink;
        sink.record(TelemetryEvent::SyncLatencyRecorded {
            latency_ms: 42,
            comment_count: 3,
            incremental: false,
        });
    }

    #[test]
    fn recording_sink_captures_sync_latency_event() {
        let sink = RecordingTelemetrySink::default();
        sink.record(TelemetryEvent::SyncLatencyRecorded {
            latency_ms: 150,
            comment_count: 10,
            incremental: false,
        });

        let events = sink.events();
        assert_eq!(events.len(), 1);

        let TelemetryEvent::SyncLatencyRecorded {
            latency_ms,
            comment_count,
            incremental,
        } = events.first().expect("events should not be empty")
        else {
            panic!(
                "expected SyncLatencyRecorded event, got {:?}",
                events.first()
            );
        };

        assert_eq!(*latency_ms, 150);
        assert_eq!(*comment_count, 10);
        assert!(!*incremental);
    }

    #[test]
    fn set_telemetry_sink_wires_sink_for_record_sync_telemetry() {
        // OnceLock: only verify events if our sink was first to be set.
        let sink = Arc::new(RecordingTelemetrySink::default());
        let was_set = set_telemetry_sink(Arc::clone(&sink) as Arc<dyn TelemetrySink>);
        record_sync_telemetry(200, 15, true);
        if was_set {
            let events = sink.events();
            assert_eq!(events.len(), 1);
            let first_event = events.first().expect("events should not be empty");
            assert!(matches!(
                first_event,
                TelemetryEvent::SyncLatencyRecorded {
                    latency_ms: 200,
                    comment_count: 15,
                    incremental: true,
                }
            ));
        }
    }

    /// Creates a `MockGitOps` with default expectations that return
    /// deterministic errors, so accidental calls produce predictable
    /// results instead of panicking.
    fn default_mock_git_ops() -> MockGitOps {
        let mut mock = MockGitOps::new();
        mock.expect_get_commit_snapshot().returning(|_, _| {
            Err(GitOperationError::RepositoryNotAvailable {
                message: "stub".to_owned(),
            })
        });
        mock.expect_get_file_at_commit().returning(|_, _| {
            Err(GitOperationError::RepositoryNotAvailable {
                message: "stub".to_owned(),
            })
        });
        mock.expect_verify_line_mapping().returning(|_| {
            Err(GitOperationError::RepositoryNotAvailable {
                message: "stub".to_owned(),
            })
        });
        mock.expect_get_parent_commits().returning(|_, _| {
            Err(GitOperationError::RepositoryNotAvailable {
                message: "stub".to_owned(),
            })
        });
        mock.expect_commit_exists().returning(|_| false);
        mock
    }

    #[test]
    fn set_git_ops_context_wires_ops_for_get() {
        let ops: Arc<dyn GitOperations> = Arc::new(default_mock_git_ops());
        let was_set = set_git_ops_context(ops, "abc123".to_owned());

        let retrieved = get_git_ops_context();
        assert!(retrieved.is_some(), "context should always be available");

        if was_set {
            let (_, head_sha) = retrieved.expect("already asserted Some");
            assert_eq!(head_sha, "abc123");
        }
    }

    #[test]
    fn set_time_travel_context_wires_context_for_get() {
        let ctx = TimeTravelContext {
            host: "github.com".to_owned(),
            owner: "octocat".to_owned(),
            repo: "hello-world".to_owned(),
            pr_number: 42,
            discovery_failure: Some("no repo found".to_owned()),
        };
        let was_set = set_time_travel_context(ctx);

        let retrieved = get_time_travel_context();
        assert!(retrieved.is_some(), "context should always be available");

        if was_set {
            let stored = retrieved.expect("already asserted Some");
            assert_eq!(stored.owner, "octocat");
            assert_eq!(stored.repo, "hello-world");
            assert_eq!(stored.pr_number, 42);
            assert_eq!(stored.discovery_failure.as_deref(), Some("no repo found"));
        }
    }

    #[test]
    fn reply_draft_config_falls_back_to_defaults() {
        let config = get_reply_draft_config();
        assert!(
            config.max_length >= 1,
            "default reply max_length should be positive"
        );
        assert!(
            !config.templates.is_empty(),
            "default reply templates should not be empty"
        );
    }

    #[test]
    fn set_reply_draft_config_normalises_zero_max_length() {
        let custom = ReplyDraftConfig::new(0, vec!["Template".to_owned()]);
        let was_set = set_reply_draft_config(custom);
        let config = get_reply_draft_config();
        assert!(config.max_length >= 1, "max_length should be normalised");
        if was_set {
            assert_eq!(config.templates, vec!["Template".to_owned()]);
        }
    }
}
