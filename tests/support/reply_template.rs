//! Shared review-comment fixtures for reply-template integration tests.

use frankie::ReviewComment;

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

#[must_use]
pub fn review_comment_with_body(body: &str) -> ReviewComment {
    ReviewComment {
        body: Some(body.to_owned()),
        ..sample_review_comment()
    }
}
