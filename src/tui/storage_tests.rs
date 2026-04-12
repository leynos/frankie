//! Tests for TUI storage helpers backed by process-global `OnceLock`s.

use std::sync::MutexGuard;

use rstest::{fixture, rstest};

use super::storage::{
    TimeTravelContext, get_commit_history_limit, get_time_travel_context, set_commit_history_limit,
    set_time_travel_context, storage_test_guard,
};

const SAMPLE_COMMIT_HISTORY_LIMIT: usize = 42;
const ALTERNATE_COMMIT_HISTORY_LIMIT: usize = 99;
const MINIMUM_COMMIT_HISTORY_LIMIT: usize = 1;
const SAMPLE_PR_NUMBER: u64 = 42;
const ALTERNATE_PR_NUMBER: u64 = 99;

fn sample_context(pr_number: u64, discovery_failure: Option<&str>) -> TimeTravelContext {
    TimeTravelContext {
        host: "github.com".to_owned(),
        owner: "octocat".to_owned(),
        repo: "hello-world".to_owned(),
        pr_number,
        discovery_failure: discovery_failure.map(str::to_owned),
    }
}

#[fixture]
fn storage_guard() -> MutexGuard<'static, ()> {
    storage_test_guard()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Verifies that the commit-history limit `OnceLock` stores a value,
/// exposes it through the getter, and rejects later writes.
#[rstest]
fn commit_history_limit_once_lock_is_sticky(storage_guard: MutexGuard<'static, ()>) {
    let _ = set_commit_history_limit(SAMPLE_COMMIT_HISTORY_LIMIT);
    assert_eq!(
        get_commit_history_limit(),
        Some(SAMPLE_COMMIT_HISTORY_LIMIT),
        "getter should return the value stored by the setter"
    );

    let _ = set_commit_history_limit(ALTERNATE_COMMIT_HISTORY_LIMIT);
    assert_eq!(
        get_commit_history_limit(),
        Some(SAMPLE_COMMIT_HISTORY_LIMIT),
        "later writes must not replace the stored commit history limit"
    );
    assert!(
        get_commit_history_limit().is_some_and(|limit| limit >= MINIMUM_COMMIT_HISTORY_LIMIT),
        "getter should expose a positive commit history limit"
    );

    drop(storage_guard);
}

/// Verifies that the time-travel-context `OnceLock` stores a value,
/// preserves discovery-failure details, and rejects later writes.
#[rstest]
fn time_travel_context_once_lock_is_sticky(storage_guard: MutexGuard<'static, ()>) {
    let context = sample_context(SAMPLE_PR_NUMBER, Some("not a git repo"));

    let _ = set_time_travel_context(context.clone());
    assert_eq!(
        get_time_travel_context(),
        Some(context.clone()),
        "getter should return the context stored by the setter"
    );

    let _ = set_time_travel_context(sample_context(ALTERNATE_PR_NUMBER, Some("other failure")));
    assert_eq!(
        get_time_travel_context(),
        Some(context.clone()),
        "later writes must not replace the stored time-travel context"
    );
    assert!(
        context.discovery_failure.as_deref() == Some("not a git repo"),
        "stored contexts should preserve discovery-failure text"
    );

    drop(storage_guard);
}
