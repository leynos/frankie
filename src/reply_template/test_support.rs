//! Test fixtures for reply-template rendering APIs.
//!
//! These helpers keep review-comment fixtures consistent across unit,
//! integration, and behavioural tests that exercise reply templating.

use crate::github::models::ReviewComment;

/// Builds a representative review comment for reply-template tests.
#[must_use]
pub fn sample_review_comment() -> ReviewComment {
    ReviewComment {
        id: 42,
        author: Some("alice".to_owned()),
        file_path: Some("src/lib.rs".to_owned()),
        line_number: Some(12),
        body: Some("Please split this into smaller functions.".to_owned()),
        ..ReviewComment::default()
    }
}

/// Clones the shared review-comment fixture with a different body.
#[must_use]
pub fn review_comment_with_body(body: &str) -> ReviewComment {
    ReviewComment {
        body: Some(body.to_owned()),
        ..sample_review_comment()
    }
}
