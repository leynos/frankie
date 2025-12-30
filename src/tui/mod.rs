//! Terminal User Interface for review listing and filtering.
//!
//! This module provides an interactive TUI for navigating and filtering
//! pull request review comments using the bubbletea-rs framework.
//!
//! # Architecture
//!
//! The TUI follows the Model-View-Update (MVU) pattern:
//!
//! - **Model**: Application state in [`app::ReviewApp`]
//! - **View**: Rendering logic in each component's `view()` method
//! - **Update**: Message-driven state transitions in `update()`
//!
//! # Modules
//!
//! - [`app`]: Main application model and entry point
//! - [`messages`]: Message types for the update loop
//! - [`state`]: Filter and cursor state management
//! - [`components`]: Reusable UI components
//! - [`input`]: Key-to-message mapping for input handling
//!
//! # Initial Data Loading
//!
//! Because bubbletea-rs's `Model` trait requires `init()` to be a static
//! function, we use a module-level storage pattern for initial data. Call
//! [`set_initial_reviews`] before starting the program, and `ReviewApp::init()`
//! will automatically retrieve the data.
//!
//! # Refresh Functionality
//!
//! Similarly, [`set_refresh_context`] must be called to enable the refresh
//! feature. This stores the necessary context (locator, token) for fetching
//! fresh review data from the GitHub API.

use std::sync::{Arc, OnceLock};

use crate::github::error::IntakeError;
use crate::github::locator::{PersonalAccessToken, PullRequestLocator};
use crate::github::models::ReviewComment;
use crate::telemetry::{NoopTelemetrySink, TelemetryEvent, TelemetrySink};

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

/// Global storage for refresh context (locator and token).
///
/// This is set before the TUI program starts to enable refresh functionality.
static REFRESH_CONTEXT: OnceLock<RefreshContext> = OnceLock::new();

/// Global storage for telemetry sink.
///
/// This is set before the TUI program starts to enable sync latency metrics.
static TELEMETRY_SINK: OnceLock<Arc<dyn TelemetrySink>> = OnceLock::new();

/// Context required to refresh review data from GitHub.
struct RefreshContext {
    locator: PullRequestLocator,
    token: PersonalAccessToken,
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

/// Gets the telemetry sink, returning a no-op sink if not configured.
fn get_telemetry_sink() -> Arc<dyn TelemetrySink> {
    TELEMETRY_SINK
        .get()
        .cloned()
        .unwrap_or_else(|| Arc::new(NoopTelemetrySink))
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
mod tests {
    use std::sync::Arc;

    use crate::telemetry::test_support::RecordingTelemetrySink;
    use crate::telemetry::{NoopTelemetrySink, TelemetryEvent, TelemetrySink};

    use super::*;

    #[test]
    fn get_telemetry_sink_returns_usable_sink() {
        // get_telemetry_sink returns a sink that implements TelemetrySink.
        // Due to OnceLock, we can't control whether it's Noop or a previously-set sink,
        // but we verify the returned sink is usable without panicking.
        let sink = get_telemetry_sink();
        sink.record(TelemetryEvent::SyncLatencyRecorded {
            latency_ms: 100,
            comment_count: 5,
            incremental: true,
        });
    }

    #[test]
    fn noop_telemetry_sink_can_record_without_panic() {
        // Verify NoopTelemetrySink (the default fallback) handles events correctly
        let sink = NoopTelemetrySink;
        sink.record(TelemetryEvent::SyncLatencyRecorded {
            latency_ms: 42,
            comment_count: 3,
            incremental: false,
        });
        // Test passes if no panic occurs
    }

    #[test]
    fn recording_sink_captures_sync_latency_event() {
        // Test that a RecordingTelemetrySink captures the exact event structure
        // that record_sync_telemetry would produce. This verifies the event
        // construction is correct, independent of OnceLock state.
        let sink = RecordingTelemetrySink::default();

        // Record directly to the sink (bypassing OnceLock)
        sink.record(TelemetryEvent::SyncLatencyRecorded {
            latency_ms: 150,
            comment_count: 10,
            incremental: false,
        });

        let events = sink.events();
        assert_eq!(events.len(), 1);

        let first_event = events.first().expect("events should not be empty");
        match first_event {
            TelemetryEvent::SyncLatencyRecorded {
                latency_ms,
                comment_count,
                incremental,
            } => {
                assert_eq!(*latency_ms, 150);
                assert_eq!(*comment_count, 10);
                assert!(!*incremental);
            }
            TelemetryEvent::SchemaVersionRecorded { .. } => {
                panic!("expected SyncLatencyRecorded event, got SchemaVersionRecorded")
            }
        }
    }

    #[test]
    fn set_telemetry_sink_wires_sink_for_record_sync_telemetry() {
        // This test verifies the wiring works when our sink is first to set.
        // Due to OnceLock, if another test ran first, our sink won't be used,
        // but the function should still not panic.
        let sink = Arc::new(RecordingTelemetrySink::default());
        let was_set = set_telemetry_sink(Arc::clone(&sink) as Arc<dyn TelemetrySink>);

        // Call the public API
        record_sync_telemetry(200, 15, true);

        // Only verify events if we were first to set the sink
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
        // If not set, test still passes - we verified no panic occurs
    }
}
