//! Deterministic discussion-thread construction for PR summaries.

use std::collections::BTreeMap;

use crate::github::ReviewComment;
use crate::verification::{CommentVerificationResult, CommentVerificationStatus, GithubCommentId};

use super::model::{GENERAL_DISCUSSION_FILE_PATH, PrDiscussionSummaryRequest};

/// Prompt-ready thread representation built from review comments.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DiscussionThread {
    /// Root comment anchoring the thread.
    pub root_comment: ReviewComment,
    /// Stable file-path grouping key.
    pub file_path: String,
    /// Comments belonging to the thread in deterministic order.
    pub comments: Vec<DiscussionThreadComment>,
    /// Stable list of related comment IDs.
    pub related_comment_ids: Vec<GithubCommentId>,
}

/// Prompt-ready comment record within a discussion thread.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DiscussionThreadComment {
    /// GitHub review-comment identifier.
    pub comment_id: GithubCommentId,
    /// Whether this comment is the thread root.
    pub is_root: bool,
    /// Reviewer login if available.
    pub author: Option<String>,
    /// Normalized comment body text.
    pub body: String,
    /// Creation timestamp used for deterministic ordering.
    pub created_at: Option<String>,
    /// Cached verification status, if available.
    pub verification_status: Option<CommentVerificationStatus>,
}

/// Builds deterministic discussion threads from review comments.
#[must_use]
pub(crate) fn build_discussion_threads(
    request: &PrDiscussionSummaryRequest,
) -> Vec<DiscussionThread> {
    let comments_by_id: BTreeMap<u64, &ReviewComment> = request
        .review_comments()
        .iter()
        .map(|comment| (comment.id, comment))
        .collect();

    let mut grouped_ids: BTreeMap<u64, Vec<u64>> = BTreeMap::new();
    for comment in request.review_comments() {
        let mut root_id = comment.id;
        while let Some(parent_id) = comments_by_id
            .get(&root_id)
            .and_then(|current| current.in_reply_to_id)
            .filter(|parent_id| comments_by_id.contains_key(parent_id))
        {
            root_id = parent_id;
        }
        grouped_ids.entry(root_id).or_default().push(comment.id);
    }

    grouped_ids
        .into_iter()
        .filter_map(|(root_id, comment_ids)| {
            let root_comment = comments_by_id.get(&root_id).copied()?;
            let mut thread_comments: Vec<_> = comment_ids
                .into_iter()
                .filter_map(|comment_id| comments_by_id.get(&comment_id).copied())
                .collect();
            thread_comments.sort_by_key(|comment| (comment.created_at.as_deref(), comment.id));

            let comments: Vec<_> = thread_comments
                .iter()
                .map(|comment| DiscussionThreadComment {
                    comment_id: comment.id.into(),
                    is_root: comment.id == root_comment.id,
                    author: comment.author.clone(),
                    body: normalized_body(comment),
                    created_at: comment.created_at.clone(),
                    verification_status: request
                        .verification_results()
                        .get(&GithubCommentId::from(comment.id))
                        .map(CommentVerificationResult::status),
                })
                .collect();
            let related_comment_ids = comments.iter().map(|comment| comment.comment_id).collect();

            Some(DiscussionThread {
                root_comment: (*root_comment).clone(),
                file_path: root_comment
                    .file_path
                    .clone()
                    .unwrap_or_else(|| GENERAL_DISCUSSION_FILE_PATH.to_owned()),
                comments,
                related_comment_ids,
            })
        })
        .collect()
}

fn normalized_body(comment: &ReviewComment) -> String {
    let body = comment
        .body
        .as_deref()
        .unwrap_or("(no comment text)")
        .trim();
    if body.is_empty() {
        "(no comment text)".to_owned()
    } else {
        body.to_owned()
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use rstest::rstest;

    use super::build_discussion_threads;
    use crate::ai::pr_discussion_summary::PrDiscussionSummaryRequest;
    use crate::github::models::test_support::minimal_review;
    use crate::verification::{
        CommentVerificationEvidence, CommentVerificationEvidenceKind, CommentVerificationResult,
        CommentVerificationStatus, GithubCommentId,
    };

    fn thread_comments() -> Vec<crate::github::ReviewComment> {
        vec![
            crate::github::ReviewComment {
                id: 2,
                in_reply_to_id: Some(1),
                body: Some("reply".to_owned()),
                created_at: Some("2026-03-09T00:01:00Z".to_owned()),
                ..minimal_review(2, "reply", "bob")
            },
            crate::github::ReviewComment {
                id: 1,
                file_path: Some("src/main.rs".to_owned()),
                created_at: Some("2026-03-09T00:00:00Z".to_owned()),
                ..minimal_review(1, "root", "alice")
            },
        ]
    }

    #[rstest]
    fn build_discussion_threads_collapses_replies_into_one_root_thread() {
        let request = PrDiscussionSummaryRequest::new(42, None, thread_comments());

        let threads = build_discussion_threads(&request);
        let first_thread = threads.first().expect("thread root should be present");
        let first_comment = first_thread
            .comments
            .first()
            .expect("root comment should be the first item");
        let second_comment = first_thread
            .comments
            .get(1)
            .expect("reply comment should be the second item");

        assert_eq!(threads.len(), 1);
        assert_eq!(first_thread.root_comment.id, 1);
        assert_eq!(first_thread.related_comment_ids.len(), 2);
        assert_eq!(first_comment.comment_id.as_u64(), 1);
        assert_eq!(second_comment.comment_id.as_u64(), 2);
    }

    #[rstest]
    fn build_discussion_threads_uses_general_discussion_fallback_for_missing_file() {
        let request = PrDiscussionSummaryRequest::new(
            42,
            None,
            vec![crate::github::ReviewComment {
                file_path: None,
                ..minimal_review(1, "general", "alice")
            }],
        );

        let threads = build_discussion_threads(&request);
        let first_thread = threads
            .first()
            .expect("general discussion thread should exist");

        assert_eq!(first_thread.file_path, "(general discussion)");
    }

    #[rstest]
    fn build_discussion_threads_attaches_verification_status_when_available() {
        let verification = CommentVerificationResult::new(
            GithubCommentId::new(1),
            "head".to_owned(),
            CommentVerificationStatus::Verified,
            CommentVerificationEvidence {
                kind: CommentVerificationEvidenceKind::LineChanged,
                message: None,
            },
        );
        let request =
            PrDiscussionSummaryRequest::new(42, None, vec![minimal_review(1, "root", "alice")])
                .with_verification_results(HashMap::from([(
                    GithubCommentId::new(1),
                    verification,
                )]));

        let threads = build_discussion_threads(&request);
        let first_thread = threads.first().expect("verified thread should exist");
        let first_comment = first_thread
            .comments
            .first()
            .expect("verified thread should keep the root comment");

        assert_eq!(
            first_comment.verification_status,
            Some(CommentVerificationStatus::Verified)
        );
    }

    #[rstest]
    fn build_discussion_threads_treats_orphan_reply_as_its_own_root() {
        let request = PrDiscussionSummaryRequest::new(
            42,
            None,
            vec![crate::github::ReviewComment {
                id: 7,
                in_reply_to_id: Some(999),
                ..minimal_review(7, "orphan", "alice")
            }],
        );

        let threads = build_discussion_threads(&request);
        let first_thread = threads.first().expect("orphan reply thread should exist");

        assert_eq!(threads.len(), 1);
        assert_eq!(first_thread.root_comment.id, 7);
    }
}
