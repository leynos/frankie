//! Service abstractions and fallback helpers for AI rewriting.

use crate::github::IntakeError;

use super::model::{CommentRewriteOutcome, CommentRewriteRequest};

/// Shared rewrite service contract used by TUI and CLI adapters.
pub trait CommentRewriteService: Send + Sync + std::fmt::Debug {
    /// Generate rewritten text for a request.
    ///
    /// # Errors
    ///
    /// Returns [`IntakeError`] when the provider call fails.
    fn rewrite_text(&self, request: &CommentRewriteRequest) -> Result<String, IntakeError>;
}

/// Execute AI rewriting while guaranteeing a graceful fallback outcome.
#[must_use]
pub fn rewrite_with_fallback(
    service: &dyn CommentRewriteService,
    request: &CommentRewriteRequest,
) -> CommentRewriteOutcome {
    let original_text = request.source_text().to_owned();

    match service.rewrite_text(request) {
        Ok(rewritten_text) => {
            let trimmed = rewritten_text.trim();
            if trimmed.is_empty() {
                return CommentRewriteOutcome::fallback(
                    original_text,
                    "AI response was empty; keeping the original draft",
                );
            }

            CommentRewriteOutcome::generated(trimmed.to_owned())
        }
        Err(error) => {
            CommentRewriteOutcome::fallback(original_text, format!("AI request failed: {error}"))
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::ai::comment_rewrite::{CommentRewriteContext, CommentRewriteMode};
    use crate::github::IntakeError;

    use super::{CommentRewriteService, rewrite_with_fallback};

    #[derive(Debug)]
    struct StubService {
        response: Result<String, IntakeError>,
    }

    impl CommentRewriteService for StubService {
        fn rewrite_text(
            &self,
            _request: &crate::ai::comment_rewrite::CommentRewriteRequest,
        ) -> Result<String, IntakeError> {
            self.response.clone()
        }
    }

    fn sample_request() -> crate::ai::comment_rewrite::CommentRewriteRequest {
        crate::ai::comment_rewrite::CommentRewriteRequest::new(
            CommentRewriteMode::Expand,
            "Original draft",
            CommentRewriteContext::default(),
        )
    }

    #[test]
    fn rewrite_with_fallback_returns_generated_when_service_succeeds() {
        let service = StubService {
            response: Ok("Expanded text".to_owned()),
        };

        let request = sample_request();
        let outcome = rewrite_with_fallback(&service, &request);
        let crate::ai::comment_rewrite::CommentRewriteOutcome::Generated(payload) = outcome else {
            panic!("expected generated outcome");
        };

        assert_eq!(payload.rewritten_text, "Expanded text");
        assert_eq!(payload.origin_label, "AI-originated");
    }

    #[test]
    fn rewrite_with_fallback_returns_fallback_when_service_fails() {
        let service = StubService {
            response: Err(IntakeError::Network {
                message: "timeout".to_owned(),
            }),
        };

        let request = sample_request();
        let outcome = rewrite_with_fallback(&service, &request);
        let crate::ai::comment_rewrite::CommentRewriteOutcome::Fallback(payload) = outcome else {
            panic!("expected fallback outcome");
        };

        assert!(payload.reason.contains("timeout"));
        assert_eq!(payload.original_text, "Original draft");
    }

    #[test]
    fn rewrite_with_fallback_returns_fallback_for_empty_output() {
        let service = StubService {
            response: Ok("\n\t".to_owned()),
        };

        let request = sample_request();
        let outcome = rewrite_with_fallback(&service, &request);
        assert!(matches!(
            outcome,
            crate::ai::comment_rewrite::CommentRewriteOutcome::Fallback(_)
        ));
    }
}
