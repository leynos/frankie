//! Validation of AI-generated thread summary drafts against the
//! deterministic discussion threads they must describe.

use std::collections::HashSet;

use crate::github::IntakeError;
use crate::verification::GithubCommentId;

use super::super::threads::DiscussionThread;
use super::ThreadSummaryDraft;

/// Checks that drafts cover exactly the requested threads, without
/// duplicates or empty fields.
pub fn validate_drafts(
    threads: &[DiscussionThread],
    drafts: &[ThreadSummaryDraft],
) -> Result<(), IntakeError> {
    let expected_ids: HashSet<_> = threads
        .iter()
        .map(|thread| GithubCommentId::from(thread.root_comment.id))
        .collect();
    let actual_ids: HashSet<_> = drafts.iter().map(|draft| draft.root_comment_id).collect();
    validate_id_sets(&expected_ids, &actual_ids)?;

    let mut seen_ids = HashSet::new();
    drafts
        .iter()
        .try_for_each(|draft| validate_draft_entry(draft, &mut seen_ids))
}

fn build_mismatch_error(
    expected_ids: &HashSet<GithubCommentId>,
    actual_ids: &HashSet<GithubCommentId>,
) -> IntakeError {
    let missing_ids: Vec<_> = expected_ids
        .difference(actual_ids)
        .map(|id| id.as_u64().to_string())
        .collect();
    let unknown_ids: Vec<_> = actual_ids
        .difference(expected_ids)
        .map(|id| id.as_u64().to_string())
        .collect();
    let mut detail = Vec::new();
    if !missing_ids.is_empty() {
        detail.push(format!("missing root IDs: {}", missing_ids.join(", ")));
    }
    if !unknown_ids.is_empty() {
        detail.push(format!("unknown root IDs: {}", unknown_ids.join(", ")));
    }

    IntakeError::Api {
        message: format!(
            "AI summary response did not match the request threads ({})",
            detail.join("; ")
        ),
    }
}

fn validate_id_sets(
    expected_ids: &HashSet<GithubCommentId>,
    actual_ids: &HashSet<GithubCommentId>,
) -> Result<(), IntakeError> {
    if expected_ids != actual_ids {
        return Err(build_mismatch_error(expected_ids, actual_ids));
    }

    Ok(())
}

fn validate_draft_entry(
    draft: &ThreadSummaryDraft,
    seen_ids: &mut HashSet<GithubCommentId>,
) -> Result<(), IntakeError> {
    if !seen_ids.insert(draft.root_comment_id) {
        return Err(IntakeError::Api {
            message: format!(
                "AI summary response repeated thread root {}",
                draft.root_comment_id.as_u64()
            ),
        });
    }

    if draft.headline.trim().is_empty() || draft.rationale.trim().is_empty() {
        return Err(IntakeError::Api {
            message: format!(
                "AI summary response omitted headline or rationale for thread {}",
                draft.root_comment_id.as_u64()
            ),
        });
    }

    Ok(())
}
