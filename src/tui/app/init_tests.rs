//! Tests for startup initialization behaviour in the review TUI.

use std::error::Error;
use std::io;
use std::time::Duration;

use bubbletea_rs::Model;
use rstest::{fixture, rstest};

use super::sync_handlers::SYNC_INTERVAL;
use super::*;
use crate::github::models::test_support::minimal_review;

#[fixture]
fn empty_app() -> ReviewApp {
    ReviewApp::empty()
}

#[test]
fn review_app_init_returns_startup_command() {
    // Try to set initial reviews. Due to OnceLock, this may fail if another
    // test has already set the reviews. The assertions below are conditional
    // on whether we were the first to set them.
    let was_set = crate::tui::set_initial_reviews(vec![minimal_review(1, "Test", "alice")]);

    let (app, cmd) = ReviewApp::init();

    // Only verify specific review data if we were the first to set it.
    if was_set {
        assert_eq!(app.filtered_count(), 1);
        assert_eq!(app.current_selected_id(), Some(1));
    }

    // Should return a startup command regardless of review content.
    assert!(cmd.is_some());
}

#[tokio::test]
async fn review_app_init_emits_initialized_message_immediately() -> Result<(), Box<dyn Error>> {
    let (_app, cmd) = ReviewApp::init();
    let startup_cmd =
        cmd.ok_or_else(|| io::Error::other("init should return a startup command"))?;

    let result = tokio::time::timeout(Duration::from_millis(250), startup_cmd).await?;
    let msg = result.ok_or_else(|| io::Error::other("startup command should return a message"))?;

    let app_msg = msg.downcast_ref::<AppMsg>();
    if !matches!(app_msg, Some(AppMsg::Initialized)) {
        return Err(io::Error::other("startup command should emit AppMsg::Initialized").into());
    }

    Ok(())
}

#[rstest]
#[tokio::test]
async fn initialized_message_arms_sync_timer(
    mut empty_app: ReviewApp,
) -> Result<(), Box<dyn Error>> {
    tokio::time::pause();

    let cmd = empty_app
        .handle_message(&AppMsg::Initialized)
        .ok_or_else(|| {
            io::Error::other("initialized message should return a sync timer command")
        })?;

    tokio::time::advance(Duration::from_secs(SYNC_INTERVAL.as_secs() + 1)).await;
    let result = cmd.await;
    let msg = result.ok_or_else(|| io::Error::other("sync timer command should emit a message"))?;

    let app_msg = msg.downcast_ref::<AppMsg>();
    if !matches!(app_msg, Some(AppMsg::SyncTick)) {
        return Err(
            io::Error::other("initialized message should arm the background sync timer").into(),
        );
    }

    Ok(())
}

#[rstest]
#[test]
fn initialized_message_is_one_shot(mut empty_app: ReviewApp) {
    assert!(
        empty_app.handle_message(&AppMsg::Initialized).is_some(),
        "first initialized message should arm the sync timer"
    );
    assert!(
        empty_app.handle_message(&AppMsg::Initialized).is_none(),
        "subsequent initialized messages should be ignored"
    );
}

#[test]
fn init_returns_expected_commands_and_state_regardless_of_prior_reviews() {
    // OnceLock may retain reviews from prior tests - this test verifies
    // invariants that hold regardless of initial review data.
    let (app, cmd) = ReviewApp::init();

    // Should return a startup command regardless of review count.
    assert!(cmd.is_some());
    // App should not be in loading state initially.
    assert!(!app.loading);
    // No error initially.
    assert!(app.error.is_none());
    // Help should not be shown initially.
    assert!(!app.show_help);
}
