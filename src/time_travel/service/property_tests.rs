//! Property tests for shared time-travel service invariants.

use std::sync::{Mutex, MutexGuard};

use chrono::Utc;
use proptest::prelude::*;

use super::{TimeTravelNavigationDirection, load_time_travel_state, navigate_time_travel_state};
use crate::local::{
    CommitMetadata, CommitSha, CommitSnapshot, GitOperationError, GitOperations,
    LineMappingRequest, LineMappingVerification, RepoFilePath,
};
use crate::time_travel::{TimeTravelInitParams, TimeTravelParams, TimeTravelState};

#[derive(Debug, Default)]
struct PropertyGitOps {
    observed_limit: Mutex<Option<usize>>,
}

impl PropertyGitOps {
    fn observed_limit(&self) -> Option<usize> {
        *lock_or_recover(&self.observed_limit)
    }
}

impl GitOperations for PropertyGitOps {
    fn get_commit_snapshot(
        &self,
        sha: &CommitSha,
        file_path: Option<&RepoFilePath>,
    ) -> Result<CommitSnapshot, GitOperationError> {
        Ok(property_snapshot(
            sha,
            file_path.map_or("src/main.rs", RepoFilePath::as_str),
        ))
    }

    fn get_file_at_commit(
        &self,
        sha: &CommitSha,
        _file_path: &RepoFilePath,
    ) -> Result<String, GitOperationError> {
        Err(GitOperationError::CommitAccessFailed {
            sha: sha.clone(),
            message: "not used by service property tests".to_owned(),
        })
    }

    fn verify_line_mapping(
        &self,
        request: &LineMappingRequest,
    ) -> Result<LineMappingVerification, GitOperationError> {
        Ok(LineMappingVerification::exact(request.line))
    }

    fn get_parent_commits(
        &self,
        _sha: &CommitSha,
        limit: usize,
    ) -> Result<Vec<CommitSha>, GitOperationError> {
        *lock_or_recover(&self.observed_limit) = Some(limit);
        Ok(property_history(limit.max(1)))
    }

    fn commit_exists(&self, _sha: &CommitSha) -> bool {
        true
    }
}

fn lock_or_recover<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn property_history(len: usize) -> Vec<CommitSha> {
    (0..len)
        .map(|index| CommitSha::new(format!("sha{index:012}")))
        .collect()
}

fn property_snapshot(sha: &CommitSha, file_path: &str) -> CommitSnapshot {
    let metadata = CommitMetadata::new(
        sha.as_str().to_owned(),
        format!("Commit {}", sha.as_str()),
        "Alice".to_owned(),
        Utc::now(),
    );
    CommitSnapshot::with_file_content(metadata, file_path.to_owned(), "fn main() {}".to_owned())
}

fn property_state(len: usize, index: usize) -> Result<TimeTravelState, TestCaseError> {
    let history = property_history(len);
    let Some(sha) = history.get(index) else {
        return Err(TestCaseError::fail(
            "generated index should be within history bounds",
        ));
    };
    Ok(TimeTravelState::new(TimeTravelInitParams {
        snapshot: property_snapshot(sha, "src/main.rs"),
        file_path: RepoFilePath::new("src/main.rs".to_owned()),
        original_line: Some(10),
        line_mapping: None,
        commit_history: history,
        current_index: index,
    }))
}

fn history_index_strategy() -> impl Strategy<Value = (usize, usize)> {
    (1usize..20).prop_flat_map(|len| (Just(len), 0usize..len))
}

proptest! {
    #[test]
    fn load_clamps_commit_history_limit_to_at_least_one(limit in 0usize..200) {
        let git_ops = PropertyGitOps::default();
        let params = TimeTravelParams::new(
            CommitSha::new("sha000000000000".to_owned()),
            RepoFilePath::new("src/main.rs".to_owned()),
            None,
        );

        let state = load_time_travel_state(&git_ops, &params, None, limit)
            .map_err(|error| TestCaseError::fail(format!("load should succeed: {error}")))?;

        prop_assert_eq!(git_ops.observed_limit(), Some(limit.max(1)));
        prop_assert!(state.commit_count() >= 1);
    }

    #[test]
    fn navigation_target_index_stays_within_history_bounds(
        (len, index) in history_index_strategy()
    ) {
        let git_ops = PropertyGitOps::default();
        let state = property_state(len, index)?;

        if state.can_go_next() {
            let navigated = navigate_time_travel_state(
                &git_ops,
                &state,
                TimeTravelNavigationDirection::Next,
                None,
            )
            .map_err(|error| TestCaseError::fail(format!("next should succeed: {error}")))?
            .ok_or_else(|| TestCaseError::fail("next should return a state"))?;

            prop_assert_eq!(navigated.current_index(), index.saturating_sub(1));
            prop_assert!(navigated.current_index() < navigated.commit_count());
        }

        if state.can_go_previous() {
            let navigated = navigate_time_travel_state(
                &git_ops,
                &state,
                TimeTravelNavigationDirection::Previous,
                None,
            )
            .map_err(|error| TestCaseError::fail(format!("previous should succeed: {error}")))?
            .ok_or_else(|| TestCaseError::fail("previous should return a state"))?;

            prop_assert_eq!(navigated.current_index(), index + 1);
            prop_assert!(navigated.current_index() < navigated.commit_count());
        }
    }
}
