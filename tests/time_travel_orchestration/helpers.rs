//! Shared fixtures and utilities for time-travel orchestration scenarios.

use std::collections::HashMap;
use std::sync::{Mutex, MutexGuard};

use chrono::Utc;
use frankie::local::{
    CommitMetadata, CommitSha, CommitSnapshot, GitOperationError, GitOperations,
    LineMappingRequest, LineMappingVerification, RepoFilePath,
};
use frankie::time_travel::{
    TimeTravelInitParams, TimeTravelNavigationDirection, TimeTravelState,
    navigate_time_travel_state,
};
use rstest::fixture;
use rstest_bdd::Slot;
use rstest_bdd_macros::ScenarioState;

pub type StepError = &'static str;
pub type StepResult = Result<(), StepError>;

#[derive(Debug, Default)]
pub struct ScriptedGitOps {
    snapshots: Mutex<HashMap<String, Result<CommitSnapshot, GitOperationError>>>,
    commit_history: Mutex<Vec<CommitSha>>,
    line_mapping: Mutex<Option<LineMappingVerification>>,
    snapshot_requests: Mutex<Vec<String>>,
}

impl ScriptedGitOps {
    pub fn with_snapshot(self, sha: &str, snapshot: CommitSnapshot) -> Self {
        lock_or_recover(&self.snapshots).insert(sha.to_owned(), Ok(snapshot));
        self
    }

    pub fn with_snapshot_error(self, sha: &str, error: GitOperationError) -> Self {
        lock_or_recover(&self.snapshots).insert(sha.to_owned(), Err(error));
        self
    }

    pub fn with_commit_history(self, commit_history: Vec<CommitSha>) -> Self {
        *lock_or_recover(&self.commit_history) = commit_history;
        self
    }

    pub fn with_line_mapping(self, line_mapping: Option<LineMappingVerification>) -> Self {
        *lock_or_recover(&self.line_mapping) = line_mapping;
        self
    }

    pub fn requested_snapshot_count(&self) -> Result<usize, StepError> {
        self.snapshot_requests
            .lock()
            .map(|requests| requests.len())
            .map_err(|_| "snapshot request mutex should lock")
    }
}

impl GitOperations for ScriptedGitOps {
    fn get_commit_snapshot(
        &self,
        sha: &CommitSha,
        _file_path: Option<&RepoFilePath>,
    ) -> Result<CommitSnapshot, GitOperationError> {
        lock_or_recover(&self.snapshot_requests).push(sha.as_str().to_owned());
        lock_or_recover(&self.snapshots)
            .get(sha.as_str())
            .cloned()
            .unwrap_or_else(|| Err(GitOperationError::CommitNotFound { sha: sha.clone() }))
    }

    fn get_file_at_commit(
        &self,
        sha: &CommitSha,
        _file_path: &RepoFilePath,
    ) -> Result<String, GitOperationError> {
        Err(GitOperationError::CommitAccessFailed {
            sha: sha.clone(),
            message: "not used in time-travel orchestration tests".to_owned(),
        })
    }

    fn verify_line_mapping(
        &self,
        request: &LineMappingRequest,
    ) -> Result<LineMappingVerification, GitOperationError> {
        Ok(lock_or_recover(&self.line_mapping)
            .clone()
            .unwrap_or_else(|| LineMappingVerification::exact(request.line)))
    }

    fn get_parent_commits(
        &self,
        _sha: &CommitSha,
        limit: usize,
    ) -> Result<Vec<CommitSha>, GitOperationError> {
        Ok(lock_or_recover(&self.commit_history)
            .iter()
            .take(limit)
            .cloned()
            .collect())
    }

    fn commit_exists(&self, _sha: &CommitSha) -> bool {
        true
    }
}

pub fn lock_or_recover<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

#[derive(ScenarioState, Default)]
pub struct TimeTravelOrchestrationWorld {
    pub commit_sha: Slot<String>,
    pub file_path: Slot<String>,
    pub line_number: Slot<Option<u32>>,
    pub head_sha: Slot<Option<CommitSha>>,
    pub commit_history_limit: Slot<usize>,
    pub git_ops: Slot<ScriptedGitOps>,
    pub load_result: Slot<Result<TimeTravelState, GitOperationError>>,
    pub navigation_result: Slot<Result<Option<TimeTravelState>, GitOperationError>>,
    pub loaded_state: Slot<TimeTravelState>,
}

#[fixture]
pub fn state() -> TimeTravelOrchestrationWorld {
    TimeTravelOrchestrationWorld::default()
}

pub fn build_snapshot(sha: &str, message: &str) -> CommitSnapshot {
    let metadata = CommitMetadata::new(
        sha.to_owned(),
        message.to_owned(),
        "Alice".to_owned(),
        Utc::now(),
    );
    CommitSnapshot::with_file_content(
        metadata,
        "src/main.rs".to_owned(),
        "fn main() {}".to_owned(),
    )
}

pub fn build_loaded_state(
    index: usize,
    original_line: Option<u32>,
) -> Result<TimeTravelState, StepError> {
    let commit_history = vec![
        CommitSha::new("abc1234567890".to_owned()),
        CommitSha::new("def5678901234".to_owned()),
        CommitSha::new("ghi9012345678".to_owned()),
    ];
    let snapshot_sha = commit_history
        .get(index)
        .ok_or("requested time-travel index should exist")?
        .as_str()
        .to_owned();

    Ok(TimeTravelState::new(TimeTravelInitParams {
        snapshot: build_snapshot(&snapshot_sha, "Loaded state"),
        file_path: RepoFilePath::new("src/main.rs".to_owned()),
        original_line,
        line_mapping: Some(LineMappingVerification::exact(10)),
        commit_history,
        current_index: index,
    }))
}

pub fn read_navigation_state<T>(
    state: &TimeTravelOrchestrationWorld,
    map: impl FnOnce(&TimeTravelState) -> T,
) -> Result<T, StepError> {
    state
        .navigation_result
        .with_ref(|result| match result {
            Ok(Some(navigation_state)) => Ok(map(navigation_state)),
            Ok(None) => Err("navigation should have returned a state"),
            Err(_) => Err("navigation should have succeeded"),
        })
        .ok_or("navigation_result should be set")?
}

pub fn read_load_result<T>(
    state: &TimeTravelOrchestrationWorld,
    map: impl FnOnce(&TimeTravelState) -> T,
) -> Result<T, StepError> {
    state
        .load_result
        .with_ref(|result| {
            result
                .as_ref()
                .map_or(Err("load should have succeeded"), |loaded_state| {
                    Ok(map(loaded_state))
                })
        })
        .ok_or("load_result should be set")?
}

pub fn navigate(
    state: &TimeTravelOrchestrationWorld,
    direction: TimeTravelNavigationDirection,
) -> StepResult {
    let loaded_state = state
        .loaded_state
        .with_ref(Clone::clone)
        .ok_or("loaded_state should be set")?;
    let head_sha = state.head_sha.with_ref(Clone::clone).unwrap_or(None);
    let result = state
        .git_ops
        .with_ref(|git_ops| {
            navigate_time_travel_state(git_ops, &loaded_state, direction, head_sha.as_ref())
        })
        .ok_or("git_ops should be configured")?;
    state.navigation_result.set(result);
    Ok(())
}

pub fn assert_eq_step<T: PartialEq>(actual: &T, expected: &T, msg: &'static str) -> StepResult {
    if actual == expected { Ok(()) } else { Err(msg) }
}
