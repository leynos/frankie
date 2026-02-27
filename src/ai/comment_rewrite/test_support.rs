//! Test-support utilities for AI comment rewrite flows.

use crate::github::IntakeError;

use super::{CommentRewriteRequest, CommentRewriteService};

/// Deterministic rewrite-service stub used by unit and behavioural tests.
#[derive(Debug, Clone)]
pub struct StubCommentRewriteService {
    response: Result<String, IntakeError>,
}

impl StubCommentRewriteService {
    /// Creates a stub that always returns the provided rewritten text.
    #[must_use]
    pub fn success(rewritten_text: impl Into<String>) -> Self {
        Self {
            response: Ok(rewritten_text.into()),
        }
    }

    /// Creates a stub that always returns the provided error.
    #[must_use]
    pub const fn failure(error: IntakeError) -> Self {
        Self {
            response: Err(error),
        }
    }
}

impl CommentRewriteService for StubCommentRewriteService {
    fn rewrite_text(&self, _request: &CommentRewriteRequest) -> Result<String, IntakeError> {
        self.response.clone()
    }
}
