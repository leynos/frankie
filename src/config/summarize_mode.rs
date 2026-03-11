//! Summary-mode helpers for configuration validation and mode detection.

use crate::config::FrankieConfig;
use crate::github::error::IntakeError;

/// Returns whether summary mode is enabled.
#[must_use]
pub(crate) const fn is_summarize_discussions_mode(config: &FrankieConfig) -> bool {
    config.summarize_discussions
}

pub(crate) fn validate_summary_mode_compatibility(
    config: &FrankieConfig,
) -> Result<(), IntakeError> {
    if !is_summarize_discussions_mode(config) {
        return Ok(());
    }

    if config.verify_resolutions {
        return Err(IntakeError::Configuration {
            message: concat!(
                "--summarize-discussions cannot be combined with ",
                "--verify-resolutions"
            )
            .to_owned(),
        });
    }

    if should_ai_rewrite(config) {
        return Err(IntakeError::Configuration {
            message: concat!(
                "--summarize-discussions cannot be combined with AI rewrite ",
                "flags; remove --ai-rewrite-mode/--ai-rewrite-text"
            )
            .to_owned(),
        });
    }

    if should_export_comments(config) {
        return Err(IntakeError::Configuration {
            message: "--summarize-discussions cannot be combined with --export".to_owned(),
        });
    }

    if config.tui {
        return Err(IntakeError::Configuration {
            message: "--summarize-discussions cannot be combined with --tui".to_owned(),
        });
    }

    Ok(())
}

const fn should_export_comments(config: &FrankieConfig) -> bool {
    config.export.is_some()
}

fn should_ai_rewrite(config: &FrankieConfig) -> bool {
    rewrite_mode_present(config) || rewrite_text_present(config)
}

fn rewrite_mode_present(config: &FrankieConfig) -> bool {
    non_empty_trimmed(config.ai_rewrite_mode.as_deref())
}

fn rewrite_text_present(config: &FrankieConfig) -> bool {
    non_empty_trimmed(config.ai_rewrite_text.as_deref())
}

fn non_empty_trimmed(value: Option<&str>) -> bool {
    value.is_some_and(|text| !text.trim().is_empty())
}
