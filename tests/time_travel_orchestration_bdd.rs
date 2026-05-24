//! Behavioural tests for public time-travel orchestration services.
//!
//! These scenarios exercise the shared `frankie::time_travel` load and
//! navigation APIs without importing `frankie::tui`.

#[path = "time_travel_orchestration/helpers.rs"]
mod time_travel_orchestration_helpers;
#[path = "time_travel_orchestration/steps.rs"]
mod time_travel_orchestration_steps;

use rstest_bdd_macros::scenario;
use time_travel_orchestration_helpers::{TimeTravelOrchestrationWorld, state};

#[scenario(
    path = "tests/features/time_travel_orchestration.feature",
    name = "Load initial state from comment metadata"
)]
fn load_initial_state_from_comment_metadata(state: TimeTravelOrchestrationWorld) {
    let _ = state;
}

#[scenario(
    path = "tests/features/time_travel_orchestration.feature",
    name = "Navigate to an older commit"
)]
fn navigate_to_an_older_commit(state: TimeTravelOrchestrationWorld) {
    let _ = state;
}

#[scenario(
    path = "tests/features/time_travel_orchestration.feature",
    name = "Navigate back to a newer commit"
)]
fn navigate_back_to_a_newer_commit(state: TimeTravelOrchestrationWorld) {
    let _ = state;
}

#[scenario(
    path = "tests/features/time_travel_orchestration.feature",
    name = "Boundary navigation returns no state"
)]
fn boundary_navigation_returns_no_state(state: TimeTravelOrchestrationWorld) {
    let _ = state;
}

#[scenario(
    path = "tests/features/time_travel_orchestration.feature",
    name = "Navigation surfaces a missing commit unchanged"
)]
fn navigation_surfaces_missing_commit_unchanged(state: TimeTravelOrchestrationWorld) {
    let _ = state;
}

#[scenario(
    path = "tests/features/time_travel_orchestration.feature",
    name = "Navigation skips line mapping when head SHA is absent"
)]
fn navigation_skips_line_mapping_when_head_sha_is_absent(state: TimeTravelOrchestrationWorld) {
    let _ = state;
}
