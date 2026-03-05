//! Verification state and cache-loading helpers for the review TUI app.

use std::sync::Arc;

use crate::persistence::ReviewCommentVerificationCache;
use crate::verification::{
    CommentVerificationEvidence, CommentVerificationResult, ResolutionVerificationService,
};

/// Verification services, cache handles, and cached results for the app.
#[derive(Debug)]
pub(crate) struct VerificationState {
    /// Service used to verify comment resolutions, when a local repo is available.
    pub(crate) service: Option<Arc<dyn ResolutionVerificationService>>,
    /// Cache for persisting verification results, when configured.
    pub(crate) cache: Option<Arc<ReviewCommentVerificationCache>>,
    /// Cached verification results keyed by GitHub comment ID.
    pub(crate) results: std::collections::HashMap<u64, CommentVerificationResult>,
    /// Monotonic request ID used to ignore stale async verification completions.
    pub(crate) next_request_id: u64,
    /// Most recent in-flight verification request ID.
    pub(crate) in_flight_request_id: Option<u64>,
}

impl Default for VerificationState {
    fn default() -> Self {
        Self {
            service: None,
            cache: None,
            results: std::collections::HashMap::new(),
            next_request_id: 1,
            in_flight_request_id: None,
        }
    }
}

impl VerificationState {
    /// Loads cached verification results for the provided comments at `head_sha`.
    ///
    /// Returns a UI-ready error message when cache loading fails.
    pub(crate) fn load_cached_review_comment_verifications(
        &mut self,
        github_comment_ids: &[u64],
        head_sha: &str,
    ) -> Option<String> {
        let cache = self.cache.as_ref()?;

        let rows = match cache.get_for_comments(github_comment_ids, head_sha) {
            Ok(rows) => rows,
            Err(error) => {
                self.results.clear();
                return Some(format!(
                    "Failed to load cached verification results: {error}"
                ));
            }
        };

        self.results = rows
            .into_iter()
            .map(|(id, row)| {
                (
                    id,
                    CommentVerificationResult::new(
                        id.into(),
                        row.target_sha,
                        row.status,
                        CommentVerificationEvidence {
                            kind: row.evidence_kind,
                            message: row.evidence_message,
                        },
                    ),
                )
            })
            .collect();

        None
    }

    /// Returns cached verification state for a comment, if available.
    #[must_use]
    pub(crate) fn verification_for_comment(
        &self,
        comment_id: u64,
    ) -> Option<&CommentVerificationResult> {
        self.results.get(&comment_id)
    }
}
