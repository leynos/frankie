//! Comment export operation for structured output.
//!
//! This module exports pull request review comments in structured formats
//! (Markdown, JSONL, or custom templates) for downstream processing by AI
//! tools or human review.

use std::io::{self, BufWriter, Write};

use camino::Utf8Path;
use cap_std::ambient_authority;
use cap_std::fs_utf8::Dir;

use frankie::{
    FrankieConfig, IntakeError, OctocrabReviewCommentGateway, PersonalAccessToken, PrUrl,
    PullRequestLocator, ReviewCommentGateway,
};

use super::export::{
    ExportFormat, ExportedComment, sort_comments, write_jsonl, write_markdown, write_template,
};

/// Context for export operations, bundling related parameters.
struct ExportContext<'a> {
    comments: &'a [ExportedComment],
    pr_url: PrUrl<'a>,
    format: ExportFormat,
    template_content: Option<&'a str>,
}

/// Exports review comments from a pull request in structured format.
///
/// # Errors
///
/// Returns an error if:
/// - The PR URL is missing or invalid
/// - The token is missing or invalid
/// - The export format is invalid
/// - The template file is missing when using template format
/// - The GitHub API call fails
/// - Writing to the output fails
pub async fn run(config: &FrankieConfig) -> Result<(), IntakeError> {
    let pr_url = config.require_pr_url()?;
    let export_format = parse_export_format(config)?;

    // Load template content if using template format
    let template_content = load_template_if_needed(config, export_format)?;

    let locator = PullRequestLocator::parse(pr_url)?;
    let token = PersonalAccessToken::new(config.resolve_token()?)?;

    // Fetch review comments
    let gateway = OctocrabReviewCommentGateway::new(&token, locator.api_base().as_str())?;
    let reviews = gateway.list_review_comments(&locator).await?;

    // Convert and sort comments
    let mut comments: Vec<ExportedComment> = reviews.iter().map(ExportedComment::from).collect();
    sort_comments(&mut comments);

    // Write to output
    let ctx = ExportContext {
        comments: &comments,
        pr_url: PrUrl::new(pr_url),
        format: export_format,
        template_content: template_content.as_deref(),
    };
    write_output(config, &ctx)
}

/// Parses the export format from configuration.
fn parse_export_format(config: &FrankieConfig) -> Result<ExportFormat, IntakeError> {
    config
        .export
        .as_ref()
        .ok_or_else(|| IntakeError::Configuration {
            message: concat!(
                "export format is required ",
                "(use --export markdown, --export jsonl, or --export template)"
            )
            .to_owned(),
        })?
        .parse()
}

/// Loads template content from file if using template format.
fn load_template_if_needed(
    config: &FrankieConfig,
    format: ExportFormat,
) -> Result<Option<String>, IntakeError> {
    if format != ExportFormat::Template {
        return Ok(None);
    }

    let template_path = config
        .template
        .as_ref()
        .ok_or_else(|| IntakeError::Configuration {
            message: "--template <PATH> is required when using --export template".to_owned(),
        })?;

    read_template_file(Utf8Path::new(template_path))
}

/// Opens the parent directory for a given path and returns the directory handle and file name.
fn open_dir_for_path<'a>(
    path: &'a Utf8Path,
    path_type: &str,
) -> Result<(Dir, &'a str), IntakeError> {
    let parent = path.parent().unwrap_or_else(|| Utf8Path::new("."));
    let file_name = path.file_name().ok_or_else(|| IntakeError::Io {
        message: format!("invalid {path_type} path '{path}': no file name"),
    })?;

    let dir = Dir::open_ambient_dir(parent, ambient_authority()).map_err(|e| IntakeError::Io {
        message: format!("failed to open directory '{parent}': {e}"),
    })?;

    Ok((dir, file_name))
}

/// Reads template content from the specified file path.
fn read_template_file(path: &Utf8Path) -> Result<Option<String>, IntakeError> {
    let (dir, file_name) = open_dir_for_path(path, "template")?;

    let content = dir.read_to_string(file_name).map_err(|e| IntakeError::Io {
        message: format!("failed to read template file '{path}': {e}"),
    })?;

    Ok(Some(content))
}

/// Writes comments to the configured output destination.
fn write_output(config: &FrankieConfig, ctx: &ExportContext<'_>) -> Result<(), IntakeError> {
    if let Some(path_str) = &config.output {
        let path = Utf8Path::new(path_str);
        let file = create_output_file(path)?;
        let mut writer = BufWriter::new(file);
        write_format(&mut writer, ctx)?;
        writer.flush().map_err(|e| IntakeError::Io {
            message: format!("failed to flush output file: {e}"),
        })?;
        Ok(())
    } else {
        let stdout = io::stdout();
        let mut writer = stdout.lock();
        write_format(&mut writer, ctx)
    }
}

/// Creates a file at the given path using capability-oriented filesystem access.
fn create_output_file(path: &Utf8Path) -> Result<cap_std::fs_utf8::File, IntakeError> {
    let (dir, file_name) = open_dir_for_path(path, "output")?;

    dir.create(file_name).map_err(|e| IntakeError::Io {
        message: format!("failed to create output file '{path}': {e}"),
    })
}

/// Writes comments in the specified format to the writer.
fn write_format<W: Write>(writer: &mut W, ctx: &ExportContext<'_>) -> Result<(), IntakeError> {
    match ctx.format {
        ExportFormat::Markdown => write_markdown(writer, ctx.comments, ctx.pr_url.as_str()),
        ExportFormat::Jsonl => write_jsonl(writer, ctx.comments),
        ExportFormat::Template => {
            // Template content is guaranteed to be present by load_template_if_needed
            let content = ctx
                .template_content
                .ok_or_else(|| IntakeError::Configuration {
                    message: "template content missing (internal error)".to_owned(),
                })?;
            write_template(writer, ctx.comments, ctx.pr_url.as_str(), content)
        }
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
        template_content: Option<&str>,
    ) -> Result<String, IntakeError> {
        let mut buffer = Vec::new();
        let ctx = ExportContext {
            comments,
            pr_url,
            format,
            template_content,
        };
        write_format(&mut buffer, &ctx)?;
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
    #[case("template", ExportFormat::Template)]
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
            None,
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
            None,
        )?;

        let parsed: serde_json::Value = serde_json::from_str(output.trim())?;
        assert_json_field_eq(&parsed, "id", 42_u64)?;
        assert_json_field_eq(&parsed, "body", "LGTM")?;
        Ok(())
    }

    #[rstest]
    fn write_format_template_writes_to_buffer() -> TestResult {
        let comments = vec![
            CommentBuilder::new(1)
                .author("alice")
                .file_path("test.rs")
                .line_number(10)
                .body("Fix this")
                .build(),
        ];

        let template = "{% for c in comments %}{{ c.reviewer }}: {{ c.body }}{% endfor %}";
        let output = write_to_string(
            &comments,
            PrUrl::new("https://example.com/pr/1"),
            ExportFormat::Template,
            Some(template),
        )?;

        assert_contains(&output, "alice: Fix this")?;
        Ok(())
    }

    #[rstest]
    fn template_format_without_content_returns_error() {
        let comments: Vec<ExportedComment> = vec![];
        let mut buffer = Vec::new();

        let ctx = ExportContext {
            comments: &comments,
            pr_url: PrUrl::new("https://example.com/pr/1"),
            format: ExportFormat::Template,
            template_content: None,
        };
        let result = write_format(&mut buffer, &ctx);

        let err = result.expect_err("should fail without template content");
        assert!(
            matches!(err, IntakeError::Configuration { ref message } if message.contains("template content missing")),
            "expected Configuration error, got: {err:?}"
        );
    }

    #[rstest]
    fn load_template_if_needed_returns_none_for_non_template_formats() -> TestResult {
        let config = FrankieConfig::default();

        let markdown_result = load_template_if_needed(&config, ExportFormat::Markdown)?;
        if markdown_result.is_some() {
            return Err("expected None for Markdown format".into());
        }

        let jsonl_result = load_template_if_needed(&config, ExportFormat::Jsonl)?;
        if jsonl_result.is_some() {
            return Err("expected None for Jsonl format".into());
        }

        Ok(())
    }

    #[rstest]
    fn load_template_if_needed_errors_when_template_path_missing() {
        let config = FrankieConfig::default();

        let result = load_template_if_needed(&config, ExportFormat::Template);
        let err = result.expect_err("should fail without template path");

        assert!(
            matches!(err, IntakeError::Configuration { ref message } if message.contains("--template")),
            "expected Configuration error mentioning --template, got: {err:?}"
        );
    }
}
