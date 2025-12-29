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

use std::sync::OnceLock;

use crate::github::error::IntakeError;
use crate::github::locator::{PersonalAccessToken, PullRequestLocator};
use crate::github::models::ReviewComment;

pub mod app;
pub mod components;
pub mod input;
pub mod messages;
pub mod state;

pub use app::ReviewApp;

/// Global storage for initial review data.
///
/// This is set before the TUI program starts and read by `ReviewApp::init()`.
static INITIAL_REVIEWS: OnceLock<Vec<ReviewComment>> = OnceLock::new();

/// Global storage for refresh context (locator and token).
///
/// This is set before the TUI program starts to enable refresh functionality.
static REFRESH_CONTEXT: OnceLock<RefreshContext> = OnceLock::new();

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
