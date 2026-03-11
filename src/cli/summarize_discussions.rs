//! CLI operation mode for PR-level discussion summary generation.

use std::io::{self, Write};
use std::time::Duration;

use frankie::ai::{
    OpenAiPrDiscussionSummaryConfig, OpenAiPrDiscussionSummaryService, PrDiscussionSummary,
    PrDiscussionSummaryRequest, PrDiscussionSummaryService,
};
use frankie::{
    FrankieConfig, IntakeError, OctocrabReviewCommentGateway, PersonalAccessToken,
    ReviewCommentGateway,
};

use super::output::io_error;
use super::pull_request_context::{fetch_pull_request_title, resolve_locator};

/// Generates and prints a PR-level discussion summary.
///
/// # Errors
///
/// Returns an error if configuration is missing, pull-request comments cannot
/// be loaded, summary generation fails, or writing output fails.
pub async fn run(config: &FrankieConfig) -> Result<(), IntakeError> {
    let locator = resolve_locator(config)?;
    let token = PersonalAccessToken::new(config.resolve_token()?)?;
    let gateway = OctocrabReviewCommentGateway::new(&token, locator.api_base().as_str())?;
    let review_comments = gateway.list_review_comments(&locator).await?;
    let pr_title = fetch_pull_request_title(&locator, &token)
        .await
        .ok()
        .flatten();
    let request =
        PrDiscussionSummaryRequest::new(locator.number().get(), pr_title, review_comments);
    let service = build_summary_service(config);
    let stdout = io::stdout();
    let mut writer = stdout.lock();

    let summary = service.summarize(&request)?;
    write_summary(&mut writer, locator.number().get(), &summary)
}

fn build_summary_service(config: &FrankieConfig) -> OpenAiPrDiscussionSummaryService {
    let service_config = OpenAiPrDiscussionSummaryConfig::new(
        config.ai_base_url.clone(),
        config.ai_model.clone(),
        config.resolve_ai_api_key(),
        Duration::from_secs(config.ai_timeout_seconds),
    );
    OpenAiPrDiscussionSummaryService::new(service_config)
}

fn write_summary<W: Write>(
    writer: &mut W,
    pr_number: u64,
    summary: &PrDiscussionSummary,
) -> Result<(), IntakeError> {
    writeln!(writer, "PR discussion summary for #{pr_number}").map_err(|error| io_error(&error))?;

    for file in &summary.files {
        writeln!(writer, "\nFile: {}", file.file_path).map_err(|error| io_error(&error))?;
        for bucket in &file.severities {
            writeln!(writer, "  Severity: {}", bucket.severity)
                .map_err(|error| io_error(&error))?;
            for item in &bucket.items {
                writeln!(writer, "    - {}", item.headline).map_err(|error| io_error(&error))?;
                writeln!(writer, "      Rationale: {}", item.rationale)
                    .map_err(|error| io_error(&error))?;
                writeln!(writer, "      Link: {}", item.tui_link)
                    .map_err(|error| io_error(&error))?;
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::write_summary;
    use frankie::FrankieConfig;
    use frankie::ai::pr_discussion_summary::test_support::StubPrDiscussionSummaryService;
    use frankie::ai::{
        DiscussionSeverity, FileDiscussionSummary, PrDiscussionSummary, SeverityBucket, TuiViewLink,
    };

    #[rstest]
    fn summary_mode_writes_grouped_output() {
        let _config = FrankieConfig {
            summarize_discussions: true,
            ..Default::default()
        };
        let _service = StubPrDiscussionSummaryService::success(PrDiscussionSummary {
            files: vec![FileDiscussionSummary {
                file_path: "src/main.rs".to_owned(),
                severities: vec![SeverityBucket {
                    severity: DiscussionSeverity::High,
                    items: vec![frankie::ai::DiscussionSummaryItem {
                        root_comment_id: 1_u64.into(),
                        related_comment_ids: vec![1_u64.into()],
                        headline: "Handle panic path".to_owned(),
                        rationale: "Review thread flagged unwrap".to_owned(),
                        severity: DiscussionSeverity::High,
                        tui_link: TuiViewLink::comment_detail(1_u64.into()),
                    }],
                }],
            }],
        });
        let mut output = Vec::new();

        write_summary(
            &mut output,
            42,
            &PrDiscussionSummary {
                files: vec![FileDiscussionSummary {
                    file_path: "src/main.rs".to_owned(),
                    severities: vec![SeverityBucket {
                        severity: DiscussionSeverity::High,
                        items: vec![frankie::ai::DiscussionSummaryItem {
                            root_comment_id: 1_u64.into(),
                            related_comment_ids: vec![1_u64.into()],
                            headline: "Handle panic path".to_owned(),
                            rationale: "Review thread flagged unwrap".to_owned(),
                            severity: DiscussionSeverity::High,
                            tui_link: TuiViewLink::comment_detail(1_u64.into()),
                        }],
                    }],
                }],
            },
        )
        .expect("summary should render");

        let text = String::from_utf8(output).expect("output should be UTF-8");
        assert!(text.contains("PR discussion summary for #42"));
        assert!(text.contains("File: src/main.rs"));
        assert!(text.contains("Severity: high"));
        assert!(text.contains("Link: frankie://review-comment/1?view=detail"));
    }
}
