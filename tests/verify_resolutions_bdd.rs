//! Behavioural tests for CLI automated resolution verification.

use std::process::Output;

use rstest::fixture;
use rstest_bdd::Slot;
use rstest_bdd_macros::{ScenarioState, scenario};
use tempfile::TempDir;
use wiremock::MockServer;

#[path = "support/runtime.rs"]
mod runtime;
#[path = "support/verify_resolutions_helpers.rs"]
mod verify_resolutions_helpers;
#[path = "steps/verify_resolutions_steps.rs"]
mod verify_resolutions_steps;

pub(crate) type StepResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

#[derive(ScenarioState, Default)]
pub(crate) struct VerifyResolutionsState {
    runtime: Slot<runtime::SharedRuntime>,
    server: Slot<MockServer>,
    database_dir: Slot<TempDir>,
    database_url: Slot<String>,
    repo_dir: Slot<TempDir>,
    repo_path: Slot<String>,
    old_sha: Slot<String>,
    head_sha: Slot<String>,
    pr_url: Slot<String>,
    comment_id: Slot<u64>,
    output: Slot<Output>,
}

#[fixture]
fn verify_state() -> VerifyResolutionsState {
    VerifyResolutionsState::default()
}

#[scenario(
    path = "tests/features/verify_resolutions.feature",
    name = "Verification marks changed lines as verified and persists results"
)]
fn verify_marks_changed_lines_as_verified(verify_state: VerifyResolutionsState) {
    let _ = verify_state;
}

#[scenario(
    path = "tests/features/verify_resolutions.feature",
    name = "Verification marks unchanged lines as unverified and persists results"
)]
fn verify_marks_unchanged_lines_as_unverified(verify_state: VerifyResolutionsState) {
    let _ = verify_state;
}

#[scenario(
    path = "tests/features/verify_resolutions.feature",
    name = "Verification marks deleted lines as verified and persists results"
)]
fn verify_marks_deleted_lines_as_verified(verify_state: VerifyResolutionsState) {
    let _ = verify_state;
}

#[scenario(
    path = "tests/features/verify_resolutions.feature",
    name = "Verification marks unknown commit mappings as unverified"
)]
fn verify_marks_unknown_commit_as_unverified(verify_state: VerifyResolutionsState) {
    let _ = verify_state;
}

#[scenario(
    path = "tests/features/verify_resolutions.feature",
    name = "Verification marks missing metadata as unverified and persists results"
)]
fn verify_marks_missing_metadata_as_unverified(verify_state: VerifyResolutionsState) {
    let _ = verify_state;
}

#[scenario(
    path = "tests/features/verify_resolutions.feature",
    name = "Verification cache reuse keeps a single row per comment and target"
)]
fn verify_cache_reuse_keeps_single_row(verify_state: VerifyResolutionsState) {
    let _ = verify_state;
}

#[scenario(
    path = "tests/features/verify_resolutions.feature",
    name = "Verification accepts a bare PR number when --repo-path is provided"
)]
fn verify_bare_pr_identifier_honours_repo_path(verify_state: VerifyResolutionsState) {
    let _ = verify_state;
}
