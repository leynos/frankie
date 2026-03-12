//! Shared domain model for PR-discussion summaries.

use std::collections::HashMap;
use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::github::ReviewComment;
use crate::verification::{CommentVerificationResult, GithubCommentId};

/// Stable fallback label for comments without a file attachment.
pub const GENERAL_DISCUSSION_FILE_PATH: &str = "(general discussion)";

/// Severity assigned to a discussion thread.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DiscussionSeverity {
    /// A blocking or materially risky problem.
    High,
    /// A meaningful issue that should be addressed soon.
    Medium,
    /// A lower-priority improvement or clean-up point.
    Low,
}

impl DiscussionSeverity {
    /// Returns the user-facing label for the severity.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::High => "high",
            Self::Medium => "medium",
            Self::Low => "low",
        }
    }

    #[must_use]
    pub(crate) const fn sort_rank(self) -> usize {
        match self {
            Self::High => 0,
            Self::Medium => 1,
            Self::Low => 2,
        }
    }
}

impl fmt::Display for DiscussionSeverity {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.label())
    }
}

/// Parse error for [`DiscussionSeverity`].
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("unsupported discussion severity '{value}': valid options are 'high', 'medium', or 'low'")]
pub struct DiscussionSeverityParseError {
    value: String,
}

impl FromStr for DiscussionSeverity {
    type Err = DiscussionSeverityParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "high" => Ok(Self::High),
            "medium" => Ok(Self::Medium),
            "low" => Ok(Self::Low),
            _ => Err(DiscussionSeverityParseError {
                value: value.to_owned(),
            }),
        }
    }
}

/// TUI view targeted by a summary link.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TuiView {
    /// Review-list comment-detail view for a selected comment.
    CommentDetail,
}

impl TuiView {
    #[must_use]
    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::CommentDetail => "detail",
        }
    }
}

/// Structured link pointing back to a TUI view.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TuiViewLink {
    /// Comment targeted by the link.
    pub comment_id: GithubCommentId,
    /// View to open for the comment.
    pub view: TuiView,
}

impl TuiViewLink {
    /// Creates a link to the comment-detail view for the provided comment.
    #[must_use]
    pub const fn comment_detail(comment_id: GithubCommentId) -> Self {
        Self {
            comment_id,
            view: TuiView::CommentDetail,
        }
    }
}

impl fmt::Display for TuiViewLink {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "frankie://review-comment/{}?view={}",
            self.comment_id.as_u64(),
            self.view.label()
        )
    }
}

/// Request payload used by PR-discussion summary services.
#[derive(Debug, Clone)]
pub struct PrDiscussionSummaryRequest {
    pr_number: u64,
    pr_title: Option<String>,
    review_comments: Vec<ReviewComment>,
    verification_results: HashMap<GithubCommentId, CommentVerificationResult>,
}

impl PrDiscussionSummaryRequest {
    /// Creates a request from PR metadata and review comments.
    #[must_use]
    pub fn new(
        pr_number: u64,
        pr_title: Option<String>,
        review_comments: Vec<ReviewComment>,
    ) -> Self {
        Self {
            pr_number,
            pr_title,
            review_comments,
            verification_results: HashMap::new(),
        }
    }

    /// Adds cached verification results as optional prompt context.
    #[must_use]
    pub fn with_verification_results(
        mut self,
        verification_results: HashMap<GithubCommentId, CommentVerificationResult>,
    ) -> Self {
        self.verification_results = verification_results;
        self
    }

    /// Pull-request number included in the prompt context.
    #[must_use]
    pub const fn pr_number(&self) -> u64 {
        self.pr_number
    }

    /// Pull-request title included in the prompt context.
    #[must_use]
    pub fn pr_title(&self) -> Option<&str> {
        self.pr_title.as_deref()
    }

    /// Review comments that make up the discussion set.
    #[must_use]
    pub const fn review_comments(&self) -> &[ReviewComment] {
        self.review_comments.as_slice()
    }

    /// Cached verification results keyed by comment ID.
    #[must_use]
    pub const fn verification_results(
        &self,
    ) -> &HashMap<GithubCommentId, CommentVerificationResult> {
        &self.verification_results
    }
}

/// Structured PR-level summary grouped by file and severity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrDiscussionSummary {
    /// File-grouped summary sections.
    pub files: Vec<FileDiscussionSummary>,
}

impl PrDiscussionSummary {
    /// Returns the total number of discussion items across all groups.
    #[must_use]
    pub fn item_count(&self) -> usize {
        self.files
            .iter()
            .map(|file| {
                file.severities
                    .iter()
                    .map(|bucket| bucket.items.len())
                    .sum::<usize>()
            })
            .sum()
    }

    /// Returns the summary item at the provided flattened item index.
    #[must_use]
    pub fn item_at(&self, target_index: usize) -> Option<&DiscussionSummaryItem> {
        self.files
            .iter()
            .flat_map(|file| file.severities.iter())
            .flat_map(|bucket| bucket.items.iter())
            .nth(target_index)
    }
}

/// File-scoped summary group.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileDiscussionSummary {
    /// File path or fallback general-discussion label.
    pub file_path: String,
    /// Severity buckets for this file.
    pub severities: Vec<SeverityBucket>,
}

/// Severity bucket within a file group.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SeverityBucket {
    /// Shared severity for the items in this bucket.
    pub severity: DiscussionSeverity,
    /// Summary items ordered deterministically within the bucket.
    pub items: Vec<DiscussionSummaryItem>,
}

/// One summarized discussion thread.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiscussionSummaryItem {
    /// Root comment anchoring the summarized thread.
    pub root_comment_id: GithubCommentId,
    /// All comments included in the summarized thread.
    pub related_comment_ids: Vec<GithubCommentId>,
    /// Short headline describing the thread.
    pub headline: String,
    /// Rationale or explanation for the headline/severity.
    pub rationale: String,
    /// Assigned severity.
    pub severity: DiscussionSeverity,
    /// Stable TUI link back to the root discussion.
    pub tui_link: TuiViewLink,
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use rstest::rstest;

    use super::{
        DiscussionSeverity, PrDiscussionSummary, PrDiscussionSummaryRequest, TuiView, TuiViewLink,
    };
    use crate::github::models::test_support::minimal_review;

    #[rstest]
    #[case("high", Some(DiscussionSeverity::High))]
    #[case("medium", Some(DiscussionSeverity::Medium))]
    #[case("low", Some(DiscussionSeverity::Low))]
    #[case("HIGH", Some(DiscussionSeverity::High))]
    #[case("urgent", None)]
    fn parse_severity(#[case] value: &str, #[case] expected: Option<DiscussionSeverity>) {
        let parsed = value.parse::<DiscussionSeverity>();

        match expected {
            Some(severity) => assert_eq!(parsed.ok(), Some(severity)),
            None => assert!(parsed.is_err(), "expected parse error for {value}"),
        }
    }

    #[test]
    fn tui_link_formats_as_uri_like_token() {
        let link = TuiViewLink::comment_detail(42_u64.into());

        assert_eq!(link.to_string(), "frankie://review-comment/42?view=detail",);
        assert_eq!(link.view, TuiView::CommentDetail);
    }

    #[test]
    fn request_defaults_verification_results_to_empty() {
        let request = PrDiscussionSummaryRequest::new(
            42,
            Some("Title".to_owned()),
            vec![minimal_review(1, "body", "alice")],
        );

        assert_eq!(request.pr_number(), 42);
        assert_eq!(request.pr_title(), Some("Title"));
        assert_eq!(request.review_comments().len(), 1);
        assert_eq!(request.verification_results(), &HashMap::new());
    }

    #[test]
    fn summary_item_accessors_use_flattened_order() {
        let summary = PrDiscussionSummary {
            files: vec![
                super::FileDiscussionSummary {
                    file_path: "src/a.rs".to_owned(),
                    severities: vec![super::SeverityBucket {
                        severity: DiscussionSeverity::High,
                        items: vec![super::DiscussionSummaryItem {
                            root_comment_id: 1_u64.into(),
                            related_comment_ids: vec![1_u64.into()],
                            headline: "Headline".to_owned(),
                            rationale: "Rationale".to_owned(),
                            severity: DiscussionSeverity::High,
                            tui_link: TuiViewLink::comment_detail(1_u64.into()),
                        }],
                    }],
                },
                super::FileDiscussionSummary {
                    file_path: "src/b.rs".to_owned(),
                    severities: vec![super::SeverityBucket {
                        severity: DiscussionSeverity::Low,
                        items: vec![super::DiscussionSummaryItem {
                            root_comment_id: 2_u64.into(),
                            related_comment_ids: vec![2_u64.into()],
                            headline: "Later".to_owned(),
                            rationale: "Later rationale".to_owned(),
                            severity: DiscussionSeverity::Low,
                            tui_link: TuiViewLink::comment_detail(2_u64.into()),
                        }],
                    }],
                },
            ],
        };

        assert_eq!(summary.item_count(), 2);
        assert_eq!(
            summary.item_at(1).map(|item| item.root_comment_id.as_u64()),
            Some(2)
        );
    }
}
