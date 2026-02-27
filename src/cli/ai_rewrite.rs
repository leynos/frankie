//! Non-interactive AI rewrite mode for reply text expansion and rewording.

use std::io::{self, Write};
use std::time::Duration;

use frankie::ai::{
    CommentRewriteContext, CommentRewriteMode, CommentRewriteOutcome, CommentRewriteRequest,
    CommentRewriteService, OpenAiCommentRewriteConfig, OpenAiCommentRewriteService,
    build_side_by_side_diff_preview,
};
use frankie::{FrankieConfig, IntakeError};

use super::output::io_error;

/// Runs non-interactive AI rewrite mode.
///
/// # Errors
///
/// Returns an error if required configuration is missing or invalid, or if
/// writing output fails.
pub fn run(config: &FrankieConfig) -> Result<(), IntakeError> {
    let service = build_rewrite_service(config);
    let stdout = io::stdout();
    let mut writer = stdout.lock();

    run_with_service(&mut writer, config, &service)
}

fn build_rewrite_service(config: &FrankieConfig) -> OpenAiCommentRewriteService {
    let service_config = OpenAiCommentRewriteConfig::new(
        config.ai_base_url.clone(),
        config.ai_model.clone(),
        config.resolve_ai_api_key(),
        Duration::from_secs(config.ai_timeout_seconds),
    );
    OpenAiCommentRewriteService::new(service_config)
}

fn run_with_service<W: Write>(
    writer: &mut W,
    config: &FrankieConfig,
    service: &dyn CommentRewriteService,
) -> Result<(), IntakeError> {
    let mode = resolve_rewrite_mode(config)?;
    let source_text = resolve_rewrite_source(config)?;
    let request = CommentRewriteRequest::new(mode, source_text, CommentRewriteContext::default());
    let outcome = rewrite_for_cli(service, &request)?;

    write_outcome(writer, mode, source_text, &outcome)
}

fn rewrite_for_cli(
    service: &dyn CommentRewriteService,
    request: &CommentRewriteRequest,
) -> Result<CommentRewriteOutcome, IntakeError> {
    match service.rewrite_text(request) {
        Ok(rewritten_text) => {
            let trimmed = rewritten_text.trim();
            if trimmed.is_empty() {
                return Ok(CommentRewriteOutcome::fallback(
                    request.source_text(),
                    "AI response was empty; keeping the original draft",
                ));
            }
            Ok(CommentRewriteOutcome::generated(trimmed.to_owned()))
        }
        Err(IntakeError::Configuration { message }) => Err(IntakeError::Configuration { message }),
        Err(error) => Ok(CommentRewriteOutcome::fallback(
            request.source_text(),
            format!("AI request failed: {error}"),
        )),
    }
}

fn resolve_rewrite_mode(config: &FrankieConfig) -> Result<CommentRewriteMode, IntakeError> {
    let mode_raw = config
        .ai_rewrite_mode
        .as_deref()
        .ok_or_else(|| IntakeError::Configuration {
            message: "--ai-rewrite-mode is required in ai-rewrite mode".to_owned(),
        })?;

    mode_raw
        .parse::<CommentRewriteMode>()
        .map_err(|error| IntakeError::Configuration {
            message: error.to_string(),
        })
}

fn resolve_rewrite_source(config: &FrankieConfig) -> Result<&str, IntakeError> {
    config
        .ai_rewrite_text
        .as_deref()
        .ok_or_else(|| IntakeError::Configuration {
            message: "--ai-rewrite-text is required in ai-rewrite mode".to_owned(),
        })
}

fn write_outcome<W: Write>(
    writer: &mut W,
    mode: CommentRewriteMode,
    original_text: &str,
    outcome: &CommentRewriteOutcome,
) -> Result<(), IntakeError> {
    writeln!(writer, "AI rewrite mode: {mode}").map_err(|error| io_error(&error))?;

    match outcome {
        CommentRewriteOutcome::Generated(generated) => {
            writeln!(writer, "Status: generated").map_err(|error| io_error(&error))?;
            writeln!(writer, "Origin: {}", generated.origin_label)
                .map_err(|error| io_error(&error))?;
            write_preview(writer, original_text, generated.rewritten_text.as_str())?;
            writeln!(writer, "\nCandidate text:\n{}", generated.rewritten_text)
                .map_err(|error| io_error(&error))?;
        }
        CommentRewriteOutcome::Fallback(fallback) => {
            writeln!(writer, "Status: fallback").map_err(|error| io_error(&error))?;
            writeln!(writer, "Reason: {}", fallback.reason).map_err(|error| io_error(&error))?;
            writeln!(
                writer,
                "\nOriginal text preserved:\n{}",
                fallback.original_text
            )
            .map_err(|error| io_error(&error))?;
        }
    }

    Ok(())
}

fn write_preview<W: Write>(
    writer: &mut W,
    original_text: &str,
    candidate_text: &str,
) -> Result<(), IntakeError> {
    let preview = build_side_by_side_diff_preview(original_text, candidate_text);
    writeln!(writer, "Preview: original || candidate").map_err(|error| io_error(&error))?;

    for (index, line) in preview.lines.iter().enumerate() {
        writeln!(
            writer,
            "{:>3}: {} || {}",
            index + 1,
            line.original,
            line.candidate
        )
        .map_err(|error| io_error(&error))?;
    }

    let changed_label = if preview.has_changes { "yes" } else { "no" };
    writeln!(writer, "Changed: {changed_label}").map_err(|error| io_error(&error))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::run_with_service;
    use frankie::ai::comment_rewrite::test_support::StubCommentRewriteService;
    use frankie::{FrankieConfig, IntakeError};

    fn base_config() -> FrankieConfig {
        FrankieConfig {
            ai_rewrite_mode: Some("expand".to_owned()),
            ai_rewrite_text: Some("Please fix this".to_owned()),
            ..Default::default()
        }
    }

    #[rstest]
    fn run_with_service_prints_generated_outcome_with_preview() {
        let config = base_config();
        let service = StubCommentRewriteService::success("Please fix this thoroughly.");
        let mut output = Vec::new();

        let result = run_with_service(&mut output, &config, &service);

        assert!(result.is_ok(), "generated flow should succeed");
        let output_text = String::from_utf8(output).unwrap_or_default();
        assert!(output_text.contains("Status: generated"));
        assert!(output_text.contains("Origin: AI-originated"));
        assert!(output_text.contains("Preview: original || candidate"));
        assert!(output_text.contains("Changed: yes"));
    }

    #[rstest]
    fn run_with_service_prints_generated_outcome_with_unchanged_preview() {
        let config = base_config();
        let service = StubCommentRewriteService::success("Please fix this");
        let mut output = Vec::new();

        let result = run_with_service(&mut output, &config, &service);

        assert!(result.is_ok(), "generated flow should succeed");
        let output_text = String::from_utf8(output).unwrap_or_default();
        assert!(output_text.contains("Status: generated"));
        assert!(output_text.contains("Origin: AI-originated"));
        assert!(output_text.contains("Preview: original || candidate"));
        assert!(output_text.contains("Changed: no"));
    }

    #[rstest]
    fn run_with_service_prints_fallback_without_failing_process() {
        let config = base_config();
        let service = StubCommentRewriteService::failure(IntakeError::Network {
            message: "timeout".to_owned(),
        });
        let mut output = Vec::new();

        let result = run_with_service(&mut output, &config, &service);

        assert!(result.is_ok(), "fallback flow should still be successful");
        let output_text = String::from_utf8(output).unwrap_or_default();
        assert!(output_text.contains("Status: fallback"));
        assert!(output_text.contains("AI request failed"));
        assert!(output_text.contains("Original text preserved"));
    }

    #[rstest]
    fn run_with_service_rejects_invalid_mode() {
        let mut config = base_config();
        config.ai_rewrite_mode = Some("invalid".to_owned());
        let service = StubCommentRewriteService::success("ignored");
        let mut output = Vec::new();

        let result = run_with_service(&mut output, &config, &service);

        assert!(
            matches!(result, Err(IntakeError::Configuration { .. })),
            "invalid mode should return configuration error"
        );
    }

    #[rstest]
    fn run_with_service_returns_error_for_configuration_failure() {
        let config = base_config();
        let service = StubCommentRewriteService::failure(IntakeError::Configuration {
            message: "AI API key is required".to_owned(),
        });
        let mut output = Vec::new();

        let result = run_with_service(&mut output, &config, &service);

        assert!(
            matches!(result, Err(IntakeError::Configuration { .. })),
            "configuration failures should be hard errors"
        );
    }
}
