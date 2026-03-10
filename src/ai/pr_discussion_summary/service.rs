//! Shared orchestration for PR-discussion summary generation.

use std::collections::{BTreeMap, HashMap, HashSet};

use crate::github::IntakeError;
use crate::verification::GithubCommentId;

use super::model::{
    DiscussionSeverity, DiscussionSummaryItem, FileDiscussionSummary, PrDiscussionSummary,
    PrDiscussionSummaryRequest, SeverityBucket, TuiViewLink,
};
use super::threads::{DiscussionThread, build_discussion_threads};

/// Shared summary service contract used by CLI and TUI adapters.
pub trait PrDiscussionSummaryService: Send + Sync + std::fmt::Debug {
    /// Generates a grouped PR-level discussion summary.
    ///
    /// # Errors
    ///
    /// Returns [`IntakeError`] when provider execution or response validation
    /// fails.
    fn summarize(
        &self,
        request: &PrDiscussionSummaryRequest,
    ) -> Result<PrDiscussionSummary, IntakeError>;
}

/// Provider-facing request assembled from deterministic discussion threads.
#[derive(Debug)]
pub(crate) struct ThreadSummaryProviderRequest<'a> {
    pub pr_number: u64,
    pub pr_title: Option<&'a str>,
    pub threads: &'a [DiscussionThread],
}

/// AI-generated draft for one summarized thread.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ThreadSummaryDraft {
    pub root_comment_id: GithubCommentId,
    pub severity: DiscussionSeverity,
    pub headline: String,
    pub rationale: String,
}

/// Internal provider contract used by concrete adapters.
pub(crate) trait ThreadSummaryProvider {
    fn summarize_threads(
        &self,
        request: &ThreadSummaryProviderRequest<'_>,
    ) -> Result<Vec<ThreadSummaryDraft>, IntakeError>;
}

pub(crate) fn summarize_with_provider(
    provider: &dyn ThreadSummaryProvider,
    request: &PrDiscussionSummaryRequest,
) -> Result<PrDiscussionSummary, IntakeError> {
    if request.review_comments().is_empty() {
        return Err(IntakeError::Configuration {
            message: "PR discussion summary requires at least one review comment".to_owned(),
        });
    }

    let threads = build_discussion_threads(request);
    let provider_request = ThreadSummaryProviderRequest {
        pr_number: request.pr_number(),
        pr_title: request.pr_title(),
        threads: threads.as_slice(),
    };
    let drafts = provider.summarize_threads(&provider_request)?;
    validate_drafts(threads.as_slice(), drafts.as_slice())?;

    Ok(group_summary_items(threads, drafts))
}

fn validate_drafts(
    threads: &[DiscussionThread],
    drafts: &[ThreadSummaryDraft],
) -> Result<(), IntakeError> {
    let expected_ids: HashSet<_> = threads
        .iter()
        .map(|thread| GithubCommentId::from(thread.root_comment.id))
        .collect();
    let actual_ids: HashSet<_> = drafts.iter().map(|draft| draft.root_comment_id).collect();
    validate_id_sets(&expected_ids, &actual_ids)?;

    let mut seen_ids = HashSet::new();
    drafts
        .iter()
        .try_for_each(|draft| validate_draft_entry(draft, &mut seen_ids))
}

fn build_mismatch_error(
    expected_ids: &HashSet<GithubCommentId>,
    actual_ids: &HashSet<GithubCommentId>,
) -> IntakeError {
    let missing_ids: Vec<_> = expected_ids
        .difference(actual_ids)
        .map(|id| id.as_u64().to_string())
        .collect();
    let unknown_ids: Vec<_> = actual_ids
        .difference(expected_ids)
        .map(|id| id.as_u64().to_string())
        .collect();
    let mut detail = Vec::new();
    if !missing_ids.is_empty() {
        detail.push(format!("missing root IDs: {}", missing_ids.join(", ")));
    }
    if !unknown_ids.is_empty() {
        detail.push(format!("unknown root IDs: {}", unknown_ids.join(", ")));
    }

    IntakeError::Api {
        message: format!(
            "AI summary response did not match the request threads ({})",
            detail.join("; ")
        ),
    }
}

fn validate_id_sets(
    expected_ids: &HashSet<GithubCommentId>,
    actual_ids: &HashSet<GithubCommentId>,
) -> Result<(), IntakeError> {
    if expected_ids != actual_ids {
        return Err(build_mismatch_error(expected_ids, actual_ids));
    }

    Ok(())
}

fn validate_draft_entry(
    draft: &ThreadSummaryDraft,
    seen_ids: &mut HashSet<GithubCommentId>,
) -> Result<(), IntakeError> {
    if !seen_ids.insert(draft.root_comment_id) {
        return Err(IntakeError::Api {
            message: format!(
                "AI summary response repeated thread root {}",
                draft.root_comment_id.as_u64()
            ),
        });
    }

    if draft.headline.trim().is_empty() || draft.rationale.trim().is_empty() {
        return Err(IntakeError::Api {
            message: format!(
                "AI summary response omitted headline or rationale for thread {}",
                draft.root_comment_id.as_u64()
            ),
        });
    }

    Ok(())
}

fn group_summary_items(
    threads: Vec<DiscussionThread>,
    drafts: Vec<ThreadSummaryDraft>,
) -> PrDiscussionSummary {
    let threads_by_root: HashMap<_, _> = threads
        .into_iter()
        .map(|thread| (GithubCommentId::from(thread.root_comment.id), thread))
        .collect();
    let mut grouped: BTreeMap<String, BTreeMap<usize, Vec<DiscussionSummaryItem>>> =
        BTreeMap::new();

    for draft in drafts {
        let Some(thread) = threads_by_root.get(&draft.root_comment_id) else {
            continue;
        };
        let item = DiscussionSummaryItem {
            root_comment_id: draft.root_comment_id,
            related_comment_ids: thread.related_comment_ids.clone(),
            headline: draft.headline.trim().to_owned(),
            rationale: draft.rationale.trim().to_owned(),
            severity: draft.severity,
            tui_link: TuiViewLink::comment_detail(draft.root_comment_id),
        };
        grouped
            .entry(thread.file_path.clone())
            .or_default()
            .entry(item.severity.sort_rank())
            .or_default()
            .push(item);
    }

    let mut files: Vec<_> = grouped
        .into_iter()
        .map(|(file_path, severity_map)| {
            let mut severities: Vec<_> = severity_map
                .into_values()
                .map(|mut items| {
                    items.sort_by_key(|item| item.root_comment_id.as_u64());
                    let severity = items
                        .first()
                        .map_or(DiscussionSeverity::Low, |item| item.severity);
                    SeverityBucket { severity, items }
                })
                .collect();
            severities.sort_by_key(|bucket| bucket.severity.sort_rank());

            FileDiscussionSummary {
                file_path,
                severities,
            }
        })
        .collect();

    files.sort_by(|left, right| {
        compare_file_groups(left.file_path.as_str(), right.file_path.as_str())
    });

    PrDiscussionSummary { files }
}

fn compare_file_groups(left: &str, right: &str) -> std::cmp::Ordering {
    match (
        left == super::model::GENERAL_DISCUSSION_FILE_PATH,
        right == super::model::GENERAL_DISCUSSION_FILE_PATH,
    ) {
        (true, false) => std::cmp::Ordering::Greater,
        (false, true) => std::cmp::Ordering::Less,
        _ => left.cmp(right),
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::{
        ThreadSummaryDraft, ThreadSummaryProvider, ThreadSummaryProviderRequest,
        summarize_with_provider,
    };
    use crate::ai::pr_discussion_summary::{DiscussionSeverity, PrDiscussionSummaryRequest};
    use crate::github::IntakeError;
    use crate::github::models::test_support::minimal_review;

    #[derive(Debug)]
    struct StubProvider {
        drafts: Result<Vec<ThreadSummaryDraft>, IntakeError>,
    }

    impl ThreadSummaryProvider for StubProvider {
        fn summarize_threads(
            &self,
            _request: &ThreadSummaryProviderRequest<'_>,
        ) -> Result<Vec<ThreadSummaryDraft>, IntakeError> {
            self.drafts.clone()
        }
    }

    fn request_with_comments() -> PrDiscussionSummaryRequest {
        PrDiscussionSummaryRequest::new(
            42,
            Some("Add summaries".to_owned()),
            vec![
                crate::github::ReviewComment {
                    file_path: Some("src/lib.rs".to_owned()),
                    ..minimal_review(2, "later", "bob")
                },
                crate::github::ReviewComment {
                    file_path: Some("src/main.rs".to_owned()),
                    ..minimal_review(1, "root", "alice")
                },
                crate::github::ReviewComment {
                    id: 3,
                    in_reply_to_id: Some(1),
                    file_path: Some("src/main.rs".to_owned()),
                    ..minimal_review(3, "reply", "carol")
                },
                crate::github::ReviewComment {
                    file_path: None,
                    ..minimal_review(4, "general", "dora")
                },
            ],
        )
    }

    #[rstest]
    fn summarize_with_provider_groups_by_file_and_severity() {
        let provider = StubProvider {
            drafts: Ok(vec![
                ThreadSummaryDraft {
                    root_comment_id: 1_u64.into(),
                    severity: DiscussionSeverity::High,
                    headline: "Fix root".to_owned(),
                    rationale: "Main issue".to_owned(),
                },
                ThreadSummaryDraft {
                    root_comment_id: 2_u64.into(),
                    severity: DiscussionSeverity::Medium,
                    headline: "Fix later".to_owned(),
                    rationale: "Secondary".to_owned(),
                },
                ThreadSummaryDraft {
                    root_comment_id: 4_u64.into(),
                    severity: DiscussionSeverity::Low,
                    headline: "General note".to_owned(),
                    rationale: "Non-file-specific".to_owned(),
                },
            ]),
        };

        let summary = summarize_with_provider(&provider, &request_with_comments())
            .expect("summary should be built");
        let first_file = summary
            .files
            .first()
            .expect("summary should include the src/lib.rs file group");
        let second_file = summary
            .files
            .get(1)
            .expect("summary should include the src/main.rs file group");
        let third_file = summary
            .files
            .get(2)
            .expect("summary should include the general discussion file group");
        let first_second_file_bucket = second_file
            .severities
            .first()
            .expect("src/main.rs should include a first severity bucket");
        let first_second_file_item = first_second_file_bucket
            .items
            .first()
            .expect("src/main.rs should include a first summary item");

        assert_eq!(summary.files.len(), 3);
        assert_eq!(first_file.file_path, "src/lib.rs");
        assert_eq!(second_file.file_path, "src/main.rs");
        assert_eq!(third_file.file_path, "(general discussion)");
        assert_eq!(
            first_second_file_item.related_comment_ids,
            vec![1_u64.into(), 3_u64.into()]
        );
        assert_eq!(
            first_second_file_item.tui_link.to_string(),
            "frankie://review-comment/1?view=detail"
        );
    }

    #[rstest]
    fn summarize_with_provider_rejects_unknown_root_id() {
        let provider = StubProvider {
            drafts: Ok(vec![ThreadSummaryDraft {
                root_comment_id: 999_u64.into(),
                severity: DiscussionSeverity::High,
                headline: "Unknown".to_owned(),
                rationale: "Unknown".to_owned(),
            }]),
        };

        let result = summarize_with_provider(&provider, &request_with_comments());

        assert!(matches!(result, Err(IntakeError::Api { .. })));
    }

    #[rstest]
    fn summarize_with_provider_rejects_missing_rationale() {
        let provider = StubProvider {
            drafts: Ok(vec![
                ThreadSummaryDraft {
                    root_comment_id: 1_u64.into(),
                    severity: DiscussionSeverity::High,
                    headline: "Fix root".to_owned(),
                    rationale: String::new(),
                },
                ThreadSummaryDraft {
                    root_comment_id: 2_u64.into(),
                    severity: DiscussionSeverity::Medium,
                    headline: "Fix later".to_owned(),
                    rationale: "Secondary".to_owned(),
                },
                ThreadSummaryDraft {
                    root_comment_id: 4_u64.into(),
                    severity: DiscussionSeverity::Low,
                    headline: "General note".to_owned(),
                    rationale: "Non-file-specific".to_owned(),
                },
            ]),
        };

        let result = summarize_with_provider(&provider, &request_with_comments());

        assert!(matches!(result, Err(IntakeError::Api { .. })));
    }

    #[rstest]
    fn summarize_with_provider_rejects_empty_comment_sets() {
        let provider = StubProvider {
            drafts: Ok(Vec::new()),
        };
        let request = PrDiscussionSummaryRequest::new(42, None, Vec::new());

        let result = summarize_with_provider(&provider, &request);

        assert!(matches!(result, Err(IntakeError::Configuration { .. })));
    }
}
