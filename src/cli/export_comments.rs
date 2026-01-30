//! Comment export operation for structured output.
//!
//! This module exports pull request review comments in structured formats
//! (Markdown or JSONL) for downstream processing by AI tools or human review.

use std::fs::File;
use std::io::{self, BufWriter, Write};

use frankie::{
    FrankieConfig, IntakeError, OctocrabReviewCommentGateway, PersonalAccessToken,
    PullRequestLocator, ReviewCommentGateway,
};

use super::export::{ExportFormat, ExportedComment, sort_comments, write_jsonl, write_markdown};

/// Exports review comments from a pull request in structured format.
///
/// # Errors
///
/// Returns an error if:
/// - The PR URL is missing or invalid
/// - The token is missing or invalid
/// - The export format is invalid
/// - The GitHub API call fails
/// - Writing to the output fails
pub async fn run(config: &FrankieConfig) -> Result<(), IntakeError> {
    let pr_url = config.require_pr_url()?;
    let export_format = parse_export_format(config)?;

    let locator = PullRequestLocator::parse(pr_url)?;
    let token = PersonalAccessToken::new(config.resolve_token()?)?;

    // Fetch review comments
    let gateway = OctocrabReviewCommentGateway::new(&token, locator.api_base().as_str())?;
    let reviews = gateway.list_review_comments(&locator).await?;

    // Convert and sort comments
    let mut comments: Vec<ExportedComment> = reviews.iter().map(ExportedComment::from).collect();
    sort_comments(&mut comments);

    // Write to output
    write_output(config, &comments, pr_url, export_format)
}

/// Parses the export format from configuration.
fn parse_export_format(config: &FrankieConfig) -> Result<ExportFormat, IntakeError> {
    config
        .export
        .as_ref()
        .ok_or_else(|| IntakeError::Configuration {
            message: "export format is required (use --export markdown or --export jsonl)"
                .to_owned(),
        })?
        .parse()
}

/// Writes comments to the configured output destination.
fn write_output(
    config: &FrankieConfig,
    comments: &[ExportedComment],
    pr_url: &str,
    format: ExportFormat,
) -> Result<(), IntakeError> {
    if let Some(path) = &config.output {
        let file = File::create(path).map_err(|e| IntakeError::Io {
            message: format!("failed to create output file '{path}': {e}"),
        })?;
        let mut writer = BufWriter::new(file);
        write_format(&mut writer, comments, pr_url, format)?;
        writer.flush().map_err(|e| IntakeError::Io {
            message: format!("failed to flush output file: {e}"),
        })?;
        Ok(())
    } else {
        let stdout = io::stdout();
        let mut writer = stdout.lock();
        write_format(&mut writer, comments, pr_url, format)
    }
}

/// Writes comments in the specified format to the writer.
fn write_format<W: Write>(
    writer: &mut W,
    comments: &[ExportedComment],
    pr_url: &str,
    format: ExportFormat,
) -> Result<(), IntakeError> {
    match format {
        ExportFormat::Markdown => write_markdown(writer, comments, pr_url),
        ExportFormat::Jsonl => write_jsonl(writer, comments),
    }
}

#[cfg(test)]
#[expect(
    clippy::indexing_slicing,
    reason = "test assertions use known JSON fields"
)]
mod tests {
    use rstest::rstest;

    use super::*;

    #[rstest]
    fn parse_export_format_returns_error_when_missing() {
        let config = FrankieConfig {
            export: None,
            ..Default::default()
        };

        let result = parse_export_format(&config);

        assert!(result.is_err());
        match result {
            Err(IntakeError::Configuration { message }) => {
                assert!(message.contains("export format is required"));
            }
            _ => panic!("expected Configuration error"),
        }
    }

    #[rstest]
    fn parse_export_format_returns_markdown() {
        let config = FrankieConfig {
            export: Some("markdown".to_owned()),
            ..Default::default()
        };

        let result = parse_export_format(&config);

        assert!(result.is_ok());
        assert_eq!(result.expect("should parse"), ExportFormat::Markdown);
    }

    #[rstest]
    fn parse_export_format_returns_jsonl() {
        let config = FrankieConfig {
            export: Some("jsonl".to_owned()),
            ..Default::default()
        };

        let result = parse_export_format(&config);

        assert!(result.is_ok());
        assert_eq!(result.expect("should parse"), ExportFormat::Jsonl);
    }

    #[rstest]
    fn parse_export_format_returns_error_for_invalid() {
        let config = FrankieConfig {
            export: Some("xml".to_owned()),
            ..Default::default()
        };

        let result = parse_export_format(&config);

        assert!(result.is_err());
        match result {
            Err(IntakeError::Configuration { message }) => {
                assert!(message.contains("unsupported export format"));
            }
            _ => panic!("expected Configuration error"),
        }
    }

    #[rstest]
    fn write_format_markdown_writes_to_buffer() {
        let mut buffer = Vec::new();
        let comments = vec![ExportedComment {
            id: 1,
            author: Some("alice".to_owned()),
            file_path: Some("test.rs".to_owned()),
            line_number: Some(10),
            original_line_number: None,
            body: Some("Fix this".to_owned()),
            diff_hunk: None,
            commit_sha: None,
            in_reply_to_id: None,
            created_at: None,
        }];

        write_format(
            &mut buffer,
            &comments,
            "https://example.com/pr/1",
            ExportFormat::Markdown,
        )
        .expect("should write");

        let output = String::from_utf8(buffer).expect("valid UTF-8");
        assert!(output.contains("# Review Comments Export"));
        assert!(output.contains("test.rs:10"));
    }

    #[rstest]
    fn write_format_jsonl_writes_to_buffer() {
        let mut buffer = Vec::new();
        let comments = vec![ExportedComment {
            id: 42,
            author: Some("bob".to_owned()),
            file_path: None,
            line_number: None,
            original_line_number: None,
            body: Some("LGTM".to_owned()),
            diff_hunk: None,
            commit_sha: None,
            in_reply_to_id: None,
            created_at: None,
        }];

        write_format(
            &mut buffer,
            &comments,
            "https://example.com/pr/1",
            ExportFormat::Jsonl,
        )
        .expect("should write");

        let output = String::from_utf8(buffer).expect("valid UTF-8");
        let parsed: serde_json::Value = serde_json::from_str(output.trim()).expect("valid JSON");
        assert_eq!(parsed["id"], 42);
        assert_eq!(parsed["body"], "LGTM");
    }
}
