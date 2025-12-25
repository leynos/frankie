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
//!
//! # Initial Data Loading
//!
//! Because bubbletea-rs's `Model` trait requires `init()` to be a static
//! function, we use a module-level storage pattern for initial data. Call
//! [`set_initial_reviews`] before starting the program, and `ReviewApp::init()`
//! will automatically retrieve the data.

use std::sync::OnceLock;

use crate::github::models::ReviewComment;

pub mod app;
pub mod components;
pub mod messages;
pub mod state;

pub use app::ReviewApp;

/// Global storage for initial review data.
///
/// This is set before the TUI program starts and consumed by `ReviewApp::init()`.
static INITIAL_REVIEWS: OnceLock<Vec<ReviewComment>> = OnceLock::new();

/// Sets the initial reviews for the TUI application.
///
/// This must be called before starting the bubbletea-rs program. The reviews
/// will be consumed by `ReviewApp::init()` when the program starts.
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

/// Takes the initial reviews from storage.
///
/// Called internally by `ReviewApp::init()`. Returns the stored reviews or
/// an empty vector if not set.
pub(crate) fn take_initial_reviews() -> Vec<ReviewComment> {
    INITIAL_REVIEWS.get().cloned().unwrap_or_default()
}
