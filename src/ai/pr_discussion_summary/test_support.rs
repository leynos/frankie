//! Test-support utilities for PR-discussion summary flows.

use crate::github::IntakeError;

use super::{PrDiscussionSummary, PrDiscussionSummaryRequest, PrDiscussionSummaryService};

/// Deterministic stub summary service used by unit and behavioural tests.
#[derive(Debug, Clone)]
pub struct StubPrDiscussionSummaryService {
    response: Result<PrDiscussionSummary, IntakeError>,
}

impl StubPrDiscussionSummaryService {
    /// Creates a stub that always returns the provided summary.
    #[must_use]
    pub const fn success(summary: PrDiscussionSummary) -> Self {
        Self {
            response: Ok(summary),
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

impl PrDiscussionSummaryService for StubPrDiscussionSummaryService {
    fn summarize(
        &self,
        _request: &PrDiscussionSummaryRequest,
    ) -> Result<PrDiscussionSummary, IntakeError> {
        self.response.clone()
    }
}
