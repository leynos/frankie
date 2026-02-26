//! Shared `rstest` fixture for `OpenAI` rewrite request tests.

use rstest::fixture;

use super::{CommentRewriteContext, CommentRewriteMode, CommentRewriteRequest};

#[fixture]
pub(crate) fn rewrite_request() -> CommentRewriteRequest {
    CommentRewriteRequest::new(
        CommentRewriteMode::Expand,
        "Please fix this",
        CommentRewriteContext::default(),
    )
}
