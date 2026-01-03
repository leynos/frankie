//! Test helpers for constructing `ReviewComment` fixtures.
//!
//! This module provides builder functions for creating `ReviewComment` instances
//! in tests, reducing boilerplate and ensuring consistency across test modules.
//!
//! # Examples
//!
//! ```
//! use frankie::github::models::test_support::{minimal_review, review_with_id};
//!
//! // Create a minimal review with just id, body, and author
//! let review = minimal_review(1, "Fix this", "alice");
//!
//! // Create a review with only an ID (uses default author/body)
//! let review = review_with_id(42);
//! ```

use super::ReviewComment;

/// Constructs a minimal `ReviewComment` with only id, body, and author set.
///
/// All other fields are set to their default values (`None`).
///
/// # Arguments
///
/// * `id` - The comment identifier
/// * `body` - The comment body text
/// * `author` - The author's login name
///
/// # Examples
///
/// ```
/// use frankie::github::models::test_support::minimal_review;
///
/// let review = minimal_review(1, "Looks good!", "alice");
/// assert_eq!(review.id, 1);
/// assert_eq!(review.body.as_deref(), Some("Looks good!"));
/// assert_eq!(review.author.as_deref(), Some("alice"));
/// ```
#[must_use]
pub fn minimal_review(id: u64, body: &str, author: &str) -> ReviewComment {
    ReviewComment {
        id,
        body: Some(body.to_owned()),
        author: Some(author.to_owned()),
        ..Default::default()
    }
}

/// Creates a `ReviewComment` with only an ID and default body/author.
///
/// The body is set to "Comment {id}" and author to "alice".
///
/// # Arguments
///
/// * `id` - The comment identifier
///
/// # Examples
///
/// ```
/// use frankie::github::models::test_support::review_with_id;
///
/// let review = review_with_id(42);
/// assert_eq!(review.id, 42);
/// assert_eq!(review.body.as_deref(), Some("Comment 42"));
/// assert_eq!(review.author.as_deref(), Some("alice"));
/// ```
#[must_use]
pub fn review_with_id(id: u64) -> ReviewComment {
    minimal_review(id, &format!("Comment {id}"), "alice")
}

/// Clones a `ReviewComment` with a different ID.
///
/// This is useful when creating variations of a base fixture.
///
/// # Arguments
///
/// * `base` - The review comment to clone
/// * `new_id` - The new ID to assign
///
/// # Examples
///
/// ```
/// use frankie::github::models::test_support::{minimal_review, review_with_different_id};
///
/// let base = minimal_review(1, "Original", "bob");
/// let variant = review_with_different_id(&base, 2);
///
/// assert_eq!(variant.id, 2);
/// assert_eq!(variant.body.as_deref(), Some("Original"));
/// assert_eq!(variant.author.as_deref(), Some("bob"));
/// ```
#[must_use]
pub fn review_with_different_id(base: &ReviewComment, new_id: u64) -> ReviewComment {
    ReviewComment {
        id: new_id,
        ..base.clone()
    }
}

/// Creates a vector of review comments with sequential IDs starting from 1.
///
/// # Arguments
///
/// * `count` - The number of review comments to create
///
/// # Examples
///
/// ```
/// use frankie::github::models::test_support::create_reviews;
///
/// let reviews = create_reviews(3);
/// assert_eq!(reviews.len(), 3);
/// assert_eq!(reviews[0].id, 1);
/// assert_eq!(reviews[1].id, 2);
/// assert_eq!(reviews[2].id, 3);
/// ```
#[must_use]
pub fn create_reviews(count: usize) -> Vec<ReviewComment> {
    (1..=count).map(|i| review_with_id(i as u64)).collect()
}
