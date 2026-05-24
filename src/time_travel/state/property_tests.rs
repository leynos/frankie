//! Property tests for time-travel state invariants.

use chrono::Utc;
use proptest::prelude::*;

use super::default_state;
use crate::local::{CommitMetadata, CommitSha, CommitSnapshot};

fn property_history(len: usize) -> Vec<CommitSha> {
    (0..len)
        .map(|index| CommitSha::new(format!("sha{index:012}")))
        .collect()
}

fn property_snapshot(sha: &CommitSha) -> CommitSnapshot {
    let metadata = CommitMetadata::new(
        sha.as_str().to_owned(),
        format!("Commit {}", sha.as_str()),
        "Alice".to_owned(),
        Utc::now(),
    );
    CommitSnapshot::with_file_content(
        metadata,
        "src/auth.rs".to_owned(),
        "fn login() {}".to_owned(),
    )
}

fn history_index_strategy() -> impl Strategy<Value = (usize, usize)> {
    (1usize..20).prop_flat_map(|len| (Just(len), 0usize..len))
}

proptest! {
    #[test]
    fn current_index_stays_within_history_bounds((len, index) in history_index_strategy()) {
        let history = property_history(len);
        let Some(sha) = history.get(index) else {
            return Err(TestCaseError::fail("generated index should be in bounds"));
        };
        let state = default_state(property_snapshot(sha), history, index);

        prop_assert!(state.current_index() < state.commit_count());
    }

    #[test]
    fn snapshot_sha_selects_consistent_history_index((len, index) in history_index_strategy()) {
        let history = property_history(len);
        let Some(sha) = history.get(index) else {
            return Err(TestCaseError::fail("generated index should be in bounds"));
        };
        let state = default_state(property_snapshot(sha), history, 0);
        let Some(current_sha) = state.commit_history().get(state.current_index()) else {
            return Err(TestCaseError::fail("current index should be in bounds"));
        };

        prop_assert_eq!(state.current_index(), index);
        prop_assert_eq!(state.snapshot().sha(), current_sha.as_str());
    }

    #[test]
    fn adjacent_navigation_is_reversible((len, index) in history_index_strategy()) {
        let history = property_history(len);
        let Some(sha) = history.get(index) else {
            return Err(TestCaseError::fail("generated index should be in bounds"));
        };
        let state = default_state(property_snapshot(sha), history.clone(), index);

        if state.can_go_previous() {
            let previous_index = state.current_index() + 1;
            let previous_state = default_state(
                property_snapshot(
                    state
                        .previous_commit_sha()
                        .ok_or_else(|| TestCaseError::fail("previous SHA should exist"))?,
                ),
                history.clone(),
                previous_index,
            );
            prop_assert_eq!(
                previous_state.next_commit_sha().map(CommitSha::as_str),
                Some(state.snapshot().sha())
            );
        }

        if state.can_go_next() {
            let next_index = state.current_index().saturating_sub(1);
            let next_state = default_state(
                property_snapshot(
                    state
                        .next_commit_sha()
                        .ok_or_else(|| TestCaseError::fail("next SHA should exist"))?,
                ),
                history,
                next_index,
            );
            prop_assert_eq!(
                next_state.previous_commit_sha().map(CommitSha::as_str),
                Some(state.snapshot().sha())
            );
        }
    }
}
