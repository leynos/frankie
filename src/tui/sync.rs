//! Incremental sync logic for merging review comment updates.
//!
//! This module provides the merge algorithm used when synchronising review
//! comments from GitHub. It preserves deterministic ordering and tracks
//! change counts for telemetry.

use std::collections::HashSet;

use crate::github::models::ReviewComment;

/// Result of merging new reviews with existing reviews.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MergeResult {
    /// Merged reviews in deterministic order (sorted by ID).
    pub reviews: Vec<ReviewComment>,
    /// Number of new comments added.
    pub added: usize,
    /// Number of comments updated (present in both old and new).
    pub updated: usize,
    /// Number of comments removed (in old but not in new).
    pub removed: usize,
}

/// Merges incoming reviews with existing reviews using ID-based tracking.
///
/// The merge algorithm:
/// 1. Builds a map of existing reviews keyed by ID
/// 2. Processes incoming reviews: inserts new, updates existing
/// 3. Removes comments that are no longer in the incoming set
/// 4. Returns results sorted by ID for deterministic ordering
///
/// # Arguments
///
/// * `existing` - Current reviews in the model
/// * `incoming` - Fresh reviews from the API
///
/// # Returns
///
/// A `MergeResult` containing the merged list and change counts.
///
/// # Examples
///
/// ```
/// use frankie::github::models::ReviewComment;
/// use frankie::tui::sync::merge_reviews;
///
/// let existing = vec![
///     ReviewComment { id: 1, body: Some("Old".into()), ..Default::default() },
/// ];
/// let incoming = vec![
///     ReviewComment { id: 1, body: Some("Updated".into()), ..Default::default() },
///     ReviewComment { id: 2, body: Some("New".into()), ..Default::default() },
/// ];
///
/// let result = merge_reviews(&existing, incoming);
/// assert_eq!(result.reviews.len(), 2);
/// assert_eq!(result.added, 1);
/// assert_eq!(result.updated, 1);
/// assert_eq!(result.removed, 0);
/// ```
#[must_use]
pub fn merge_reviews(existing: &[ReviewComment], incoming: Vec<ReviewComment>) -> MergeResult {
    let existing_ids: HashSet<u64> = existing.iter().map(|r| r.id).collect();
    let incoming_ids: HashSet<u64> = incoming.iter().map(|r| r.id).collect();

    // Count changes
    let added = incoming_ids.difference(&existing_ids).count();
    let updated = incoming_ids.intersection(&existing_ids).count();
    let removed = existing_ids.difference(&incoming_ids).count();

    // Build result from incoming (which has the fresh data)
    let mut reviews: Vec<ReviewComment> = incoming;

    // Sort by ID for deterministic ordering
    reviews.sort_by_key(|r| r.id);

    MergeResult {
        reviews,
        added,
        updated,
        removed,
    }
}

#[cfg(test)]
mod tests {
    use rstest::{fixture, rstest};

    use super::*;

    #[fixture]
    fn base_review() -> ReviewComment {
        ReviewComment {
            id: 1,
            body: Some("Test comment".to_owned()),
            author: Some("alice".to_owned()),
            file_path: Some("src/main.rs".to_owned()),
            line_number: Some(10),
            original_line_number: None,
            diff_hunk: None,
            commit_sha: None,
            in_reply_to_id: None,
            created_at: None,
            updated_at: None,
        }
    }

    fn review_with_id(id: u64) -> ReviewComment {
        ReviewComment {
            id,
            body: Some(format!("Comment {id}")),
            author: Some("alice".to_owned()),
            file_path: None,
            line_number: None,
            original_line_number: None,
            diff_hunk: None,
            commit_sha: None,
            in_reply_to_id: None,
            created_at: None,
            updated_at: None,
        }
    }

    #[rstest]
    fn merge_with_no_changes_returns_same_count(base_review: ReviewComment) {
        let existing = vec![base_review.clone()];
        let incoming = vec![base_review];

        let result = merge_reviews(&existing, incoming);

        assert_eq!(result.reviews.len(), 1);
        assert_eq!(result.added, 0);
        assert_eq!(result.updated, 1);
        assert_eq!(result.removed, 0);
    }

    #[rstest]
    fn merge_adds_new_comments(base_review: ReviewComment) {
        let existing = vec![base_review.clone()];
        let new_review = ReviewComment {
            id: 2,
            ..base_review.clone()
        };
        let incoming = vec![base_review, new_review];

        let result = merge_reviews(&existing, incoming);

        assert_eq!(result.reviews.len(), 2);
        assert_eq!(result.added, 1);
        assert_eq!(result.updated, 1);
        assert_eq!(result.removed, 0);
    }

    #[rstest]
    fn merge_removes_deleted_comments(base_review: ReviewComment) {
        let review2 = ReviewComment {
            id: 2,
            ..base_review.clone()
        };
        let existing = vec![base_review.clone(), review2];
        let incoming = vec![base_review]; // review2 removed

        let result = merge_reviews(&existing, incoming);

        assert_eq!(result.reviews.len(), 1);
        assert_eq!(result.added, 0);
        assert_eq!(result.updated, 1);
        assert_eq!(result.removed, 1);
    }

    #[rstest]
    #[expect(
        clippy::indexing_slicing,
        reason = "Test asserts length before indexing"
    )]
    fn merge_updates_existing_comments(base_review: ReviewComment) {
        let existing = vec![base_review.clone()];
        let updated_review = ReviewComment {
            body: Some("Updated body".to_owned()),
            ..base_review
        };
        let incoming = vec![updated_review.clone()];

        let result = merge_reviews(&existing, incoming);

        assert_eq!(result.reviews.len(), 1);
        assert_eq!(result.reviews[0].body, Some("Updated body".to_owned()));
        assert_eq!(result.added, 0);
        assert_eq!(result.updated, 1);
        assert_eq!(result.removed, 0);
    }

    #[rstest]
    fn merge_maintains_deterministic_order_by_id() {
        let r1 = review_with_id(3);
        let r2 = review_with_id(1);
        let r3 = review_with_id(2);

        // Incoming in random order
        let result = merge_reviews(&[], vec![r1, r2, r3]);

        // Result should be sorted by ID
        let ids: Vec<u64> = result.reviews.iter().map(|r| r.id).collect();
        assert_eq!(ids, vec![1, 2, 3]);
    }

    #[rstest]
    fn merge_with_empty_existing_adds_all() {
        let r1 = review_with_id(1);
        let r2 = review_with_id(2);

        let result = merge_reviews(&[], vec![r1, r2]);

        assert_eq!(result.reviews.len(), 2);
        assert_eq!(result.added, 2);
        assert_eq!(result.updated, 0);
        assert_eq!(result.removed, 0);
    }

    #[rstest]
    fn merge_with_empty_incoming_removes_all(base_review: ReviewComment) {
        let existing = vec![base_review];

        let result = merge_reviews(&existing, vec![]);

        assert_eq!(result.reviews.len(), 0);
        assert_eq!(result.added, 0);
        assert_eq!(result.updated, 0);
        assert_eq!(result.removed, 1);
    }

    #[rstest]
    fn merge_handles_complete_replacement() {
        let old1 = review_with_id(1);
        let old2 = review_with_id(2);
        let new1 = review_with_id(3);
        let new2 = review_with_id(4);

        let result = merge_reviews(&[old1, old2], vec![new1, new2]);

        assert_eq!(result.reviews.len(), 2);
        assert_eq!(result.added, 2);
        assert_eq!(result.updated, 0);
        assert_eq!(result.removed, 2);

        let ids: Vec<u64> = result.reviews.iter().map(|r| r.id).collect();
        assert_eq!(ids, vec![3, 4]);
    }
}
