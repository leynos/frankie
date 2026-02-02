//! Comment export operation for structured output.
//!
//! This module exports pull request review comments in structured formats
//! (Markdown or JSONL) for downstream processing by AI tools or human review.

use std::io::{self, BufWriter, Write};

use camino::Utf8Path;
use cap_std::ambient_authority;
use cap_std::fs_utf8::Dir;

use frankie::{
    FrankieConfig, IntakeError, OctocrabReviewCommentGateway, PersonalAccessToken, PrUrl,
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
    write_output(config, &comments, PrUrl::new(pr_url), export_format)
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
    pr_url: PrUrl<'_>,
    format: ExportFormat,
) -> Result<(), IntakeError> {
    if let Some(path_str) = &config.output {
        let path = Utf8Path::new(path_str);
        let file = create_output_file(path)?;
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

/// Creates a file at the given path using capability-oriented filesystem access.
fn create_output_file(path: &Utf8Path) -> Result<cap_std::fs_utf8::File, IntakeError> {
    let parent = path.parent().unwrap_or_else(|| Utf8Path::new("."));
    let file_name = path.file_name().ok_or_else(|| IntakeError::Io {
        message: format!("invalid output path '{path}': no file name"),
    })?;

    let dir = Dir::open_ambient_dir(parent, ambient_authority()).map_err(|e| IntakeError::Io {
        message: format!("failed to open directory '{parent}': {e}"),
    })?;

    dir.create(file_name).map_err(|e| IntakeError::Io {
        message: format!("failed to create output file '{path}': {e}"),
    })
}

/// Writes comments in the specified format to the writer.
fn write_format<W: Write>(
    writer: &mut W,
    comments: &[ExportedComment],
    pr_url: PrUrl<'_>,
    format: ExportFormat,
) -> Result<(), IntakeError> {
    match format {
        ExportFormat::Markdown => write_markdown(writer, comments, pr_url.as_str()),
        ExportFormat::Jsonl => write_jsonl(writer, comments),
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use frankie::export::test_helpers::{CommentBuilder, TestError, assert_contains};

    use super::*;

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    fn assert_parse_error_contains(
        config: &FrankieConfig,
        expected_msg_fragment: &str,
    ) -> Result<(), TestError> {
        let result = parse_export_format(config);
        match result {
            Err(IntakeError::Configuration { message }) => {
                if message.contains(expected_msg_fragment) {
                    Ok(())
                } else {
                    Err(format!(
                        "expected message to contain '{expected_msg_fragment}', got: {message}"
                    )
                    .into())
                }
            }
            Err(other) => Err(format!("expected Configuration error, got: {other:?}").into()),
            Ok(_) => Err("expected error but got success".to_owned().into()),
        }
    }

    fn write_to_string(
        comments: &[ExportedComment],
        pr_url: PrUrl<'_>,
        format: ExportFormat,
    ) -> Result<String, IntakeError> {
        let mut buffer = Vec::new();
        write_format(&mut buffer, comments, pr_url, format)?;
        String::from_utf8(buffer).map_err(|e| IntakeError::Io {
            message: format!("invalid UTF-8: {e}"),
        })
    }

    fn assert_json_field_eq(
        parsed: &serde_json::Value,
        field: &str,
        expected: impl Into<serde_json::Value>,
    ) -> Result<(), TestError> {
        let actual = parsed.get(field);
        let expected_val = expected.into();
        if actual == Some(&expected_val) {
            Ok(())
        } else {
            Err(
                format!("field '{field}' mismatch: expected {expected_val:?}, got {actual:?}")
                    .into(),
            )
        }
    }

    #[rstest]
    #[case("markdown", ExportFormat::Markdown)]
    #[case("jsonl", ExportFormat::Jsonl)]
    fn parse_export_format_returns_expected_format(
        #[case] input: &str,
        #[case] expected: ExportFormat,
    ) {
        let config = FrankieConfig {
            export: Some(input.to_owned()),
            ..Default::default()
        };

        let result = parse_export_format(&config).expect("should parse valid format");
        assert_eq!(result, expected);
    }

    #[rstest]
    #[case(None, "export format is required")]
    #[case(Some("xml"), "unsupported export format")]
    #[case(Some("csv"), "unsupported export format")]
    #[case(Some("yaml"), "unsupported export format")]
    fn parse_export_format_rejects_missing_or_invalid(
        #[case] input: Option<&str>,
        #[case] expected_error: &str,
    ) -> TestResult {
        let config = FrankieConfig {
            export: input.map(str::to_owned),
            ..Default::default()
        };

        assert_parse_error_contains(&config, expected_error)?;
        Ok(())
    }

    #[rstest]
    fn write_format_markdown_writes_to_buffer() -> TestResult {
        let comments = vec![
            CommentBuilder::new(1)
                .author("alice")
                .file_path("test.rs")
                .line_number(10)
                .body("Fix this")
                .build(),
        ];

        let output = write_to_string(
            &comments,
            PrUrl::new("https://example.com/pr/1"),
            ExportFormat::Markdown,
        )?;

        assert_contains(&output, "# Review Comments Export")?;
        assert_contains(&output, "test.rs:10")?;
        Ok(())
    }

    #[rstest]
    fn write_format_jsonl_writes_to_buffer() -> TestResult {
        let comments = vec![CommentBuilder::new(42).author("bob").body("LGTM").build()];

        let output = write_to_string(
            &comments,
            PrUrl::new("https://example.com/pr/1"),
            ExportFormat::Jsonl,
        )?;

        let parsed: serde_json::Value = serde_json::from_str(output.trim())?;
        assert_json_field_eq(&parsed, "id", 42_u64)?;
        assert_json_field_eq(&parsed, "body", "LGTM")?;
        Ok(())
    }
}
