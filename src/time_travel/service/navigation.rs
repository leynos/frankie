//! Navigation helpers for shared time-travel service orchestration.

use std::time::Instant;

use metrics::{counter, histogram};

use crate::local::{CommitSha, GitOperationError, GitOperations};

use super::line_mapping::{LineMappingContext, verify_line_mapping};
use super::loading::load_navigation_snapshot;
use crate::time_travel::{TimeTravelInitParams, TimeTravelState};

/// Direction for navigating within a loaded time-travel history.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeTravelNavigationDirection {
    /// Navigate to the next, more recent commit in the loaded history.
    Next,
    /// Navigate to the previous, older commit in the loaded history.
    Previous,
}

impl TimeTravelNavigationDirection {
    /// Returns whether navigation in this direction is currently possible.
    ///
    /// # Example
    ///
    /// ```
    /// use chrono::Utc;
    /// use frankie::local::{CommitMetadata, CommitSha, CommitSnapshot, RepoFilePath};
    /// use frankie::time_travel::{
    ///     TimeTravelInitParams, TimeTravelNavigationDirection, TimeTravelState,
    /// };
    ///
    /// let snapshot = CommitSnapshot::with_file_content(
    ///     CommitMetadata::new(
    ///         "abc123".to_owned(),
    ///         "Newest commit".to_owned(),
    ///         "Alice".to_owned(),
    ///         Utc::now(),
    ///     ),
    ///     "src/main.rs".to_owned(),
    ///     "fn main() {}".to_owned(),
    /// );
    /// let state = TimeTravelState::new(TimeTravelInitParams {
    ///     snapshot,
    ///     file_path: RepoFilePath::new("src/main.rs".to_owned()),
    ///     original_line: Some(7),
    ///     line_mapping: None,
    ///     commit_history: vec![
    ///         CommitSha::new("abc123".to_owned()),
    ///         CommitSha::new("def456".to_owned()),
    ///     ],
    ///     current_index: 0,
    /// });
    ///
    /// assert!(TimeTravelNavigationDirection::Previous.can_navigate(&state));
    /// assert!(!TimeTravelNavigationDirection::Next.can_navigate(&state));
    /// ```
    #[must_use]
    pub const fn can_navigate(self, state: &TimeTravelState) -> bool {
        match self {
            Self::Next => state.can_go_next(),
            Self::Previous => state.can_go_previous(),
        }
    }

    fn target_sha(self, state: &TimeTravelState) -> Option<&CommitSha> {
        match self {
            Self::Next => state.next_commit_sha(),
            Self::Previous => state.previous_commit_sha(),
        }
    }

    const fn target_index(self, current_index: usize) -> usize {
        match self {
            Self::Next => current_index.saturating_sub(1),
            Self::Previous => current_index + 1,
        }
    }
}

/// Navigates a loaded time-travel state to an adjacent commit.
///
/// Returns `Ok(None)` when the requested navigation is unavailable at the
/// current history boundary or while the state is marked as loading.
///
/// # Errors
///
/// Returns a [`GitOperationError`] when loading the target snapshot fails.
///
/// # Example
///
/// ```no_run
/// use frankie::local::CommitSha;
/// use frankie::local::GitOperations;
/// use frankie::time_travel::{
///     TimeTravelNavigationDirection, TimeTravelParams, load_time_travel_state,
///     navigate_time_travel_state,
/// };
///
/// # fn example(git_ops: &dyn GitOperations) -> Result<(), Box<dyn std::error::Error>> {
/// let params = TimeTravelParams::new(
///     CommitSha::new("abc123".to_owned()),
///     "src/main.rs".into(),
///     Some(42),
/// );
/// let state = load_time_travel_state(git_ops, &params, None, 50)?;
/// let older_state = navigate_time_travel_state(
///     git_ops,
///     &state,
///     TimeTravelNavigationDirection::Previous,
///     None,
/// )?;
///
/// if let Some(older_state) = older_state {
///     assert!(older_state.current_index() > state.current_index());
/// }
/// # Ok(())
/// # }
/// ```
pub fn navigate_time_travel_state(
    git_ops: &dyn GitOperations,
    state: &TimeTravelState,
    direction: TimeTravelNavigationDirection,
    head_sha: Option<&CommitSha>,
) -> Result<Option<TimeTravelState>, GitOperationError> {
    let started_at = Instant::now();
    counter!("time_travel_service_operations_total", "operation" => "navigate").increment(1);
    let result = (|| {
        let Some(target_sha) = navigation_target(state, direction) else {
            return Ok(None);
        };
        let snapshot = load_navigation_snapshot(git_ops, state, direction, target_sha)?;
        let line_mapping = verify_line_mapping(&LineMappingContext {
            git_ops,
            commit_sha: target_sha,
            file_path: state.file_path(),
            original_line: state.original_line(),
            head_sha,
        });

        Ok(Some(TimeTravelState::new(TimeTravelInitParams {
            snapshot,
            file_path: state.file_path().clone(),
            original_line: state.original_line(),
            line_mapping,
            commit_history: state.commit_history().to_vec(),
            current_index: direction.target_index(state.current_index()),
        })))
    })();
    histogram!("time_travel_service_operation_duration_seconds", "operation" => "navigate")
        .record(started_at.elapsed().as_secs_f64());
    match &result {
        Ok(Some(next_state)) => {
            counter!("time_travel_service_operations_success_total", "operation" => "navigate")
                .increment(1);
            tracing::info!(
                direction = ?direction,
                from_index = state.current_index(),
                to_index = next_state.current_index(),
                target_sha = next_state.snapshot().sha(),
                "navigated time-travel state"
            );
        }
        Ok(None) => {
            counter!("time_travel_service_operations_noop_total", "operation" => "navigate")
                .increment(1);
            log_navigation_noop(state, direction);
        }
        Err(_) => {}
    }
    result
}

fn log_navigation_noop(state: &TimeTravelState, direction: TimeTravelNavigationDirection) {
    tracing::debug!(
        direction = ?direction,
        from_index = state.current_index(),
        to_index = direction.target_index(state.current_index()),
        commit_count = state.commit_count(),
        loading = state.is_loading(),
        "time-travel navigation produced no state"
    );
}

fn navigation_target(
    state: &TimeTravelState,
    direction: TimeTravelNavigationDirection,
) -> Option<&CommitSha> {
    if navigation_blocked(state, direction) {
        return None;
    }

    target_sha_or_log(state, direction)
}

fn navigation_blocked(state: &TimeTravelState, direction: TimeTravelNavigationDirection) -> bool {
    let is_blocked = !direction.can_navigate(state);
    if is_blocked {
        tracing::debug!(
            direction = ?direction,
            current_index = state.current_index(),
            commit_count = state.commit_count(),
            loading = state.is_loading(),
            "time-travel navigation returned no state"
        );
    }
    is_blocked
}

fn target_sha_or_log(
    state: &TimeTravelState,
    direction: TimeTravelNavigationDirection,
) -> Option<&CommitSha> {
    direction.target_sha(state).or_else(|| {
        tracing::debug!(
            direction = ?direction,
            current_index = state.current_index(),
            commit_count = state.commit_count(),
            "time-travel navigation had no target SHA"
        );
        None
    })
}
