//! Time-travel navigation view component.
//!
//! Renders the time-travel view showing the file content at a specific commit,
//! with commit metadata and line mapping verification status.

use std::fmt::Write;

use crate::tui::state::TimeTravelState;

use super::text_truncate::truncate_to_height;
use super::text_wrap::wrap_code_block;

/// Placeholder shown when no time-travel state is available.
const NO_STATE_PLACEHOLDER: &str = "(No time-travel state available)";

/// Placeholder shown when loading.
const LOADING_PLACEHOLDER: &str = "Loading commit snapshot...";

/// Context for rendering the time-travel view.
#[derive(Debug, Clone)]
pub(crate) struct TimeTravelViewContext<'a> {
    /// The time-travel state, if available.
    pub state: Option<&'a TimeTravelState>,
    /// Maximum width in columns.
    pub max_width: usize,
    /// Maximum height in lines (0 = unlimited).
    pub max_height: usize,
}

/// Component responsible for rendering the time-travel navigation view.
#[derive(Debug, Default)]
pub(crate) struct TimeTravelViewComponent;

impl TimeTravelViewComponent {
    /// Renders the time-travel view as a string.
    #[must_use]
    pub(crate) fn view(ctx: &TimeTravelViewContext<'_>) -> String {
        let Some(state) = ctx.state else {
            return format!("{NO_STATE_PLACEHOLDER}\n");
        };

        // Handle error state
        if let Some(error) = state.error_message() {
            return format!("Error: {error}\n");
        }

        // Handle loading state
        if state.is_loading() {
            return format!("{LOADING_PLACEHOLDER}\n");
        }

        let mut output = String::new();

        // Render header with commit info
        output.push_str(&render_commit_header(state));
        output.push('\n');

        // Render line mapping status
        output.push_str(&render_line_mapping(state));
        output.push('\n');

        // Render navigation indicator
        output.push_str(&render_navigation_indicator(state));
        output.push('\n');
        output.push('\n');

        // Render file content with height limit accounting for chrome
        // Reserve 4 lines for: header, line mapping, navigation, blank line
        let chrome_height = 4;
        let content_height = ctx.max_height.saturating_sub(chrome_height);

        let content = if content_height > 0 {
            let full_content = render_file_content(state, ctx.max_width);
            let mut truncated = full_content;
            truncate_to_height(&mut truncated, content_height);
            truncated
        } else {
            render_file_content(state, ctx.max_width)
        };
        output.push_str(&content);

        output
    }
}

fn render_commit_header(state: &TimeTravelState) -> String {
    let snapshot = state.snapshot();
    let short_sha = snapshot.short_sha();
    let message = snapshot.message();

    format!("Commit: {short_sha}  \"{message}\"")
}

fn render_line_mapping(state: &TimeTravelState) -> String {
    let file_path = state.file_path();

    match (state.original_line(), state.line_mapping()) {
        (Some(_line), Some(mapping)) => {
            format!("File: {file_path}  {}", mapping.display())
        }
        (Some(line), None) => {
            format!("File: {file_path}  Line {line}")
        }
        (None, _) => {
            format!("File: {file_path}")
        }
    }
}

fn render_navigation_indicator(state: &TimeTravelState) -> String {
    let current = state.current_index() + 1;
    let total = state.commit_count();

    let prev_indicator = if state.can_go_previous() {
        "[h] Previous"
    } else {
        "    --------"
    };

    let next_indicator = if state.can_go_next() {
        "[l] Next"
    } else {
        "--------"
    };

    format!("Commit {current}/{total}  {prev_indicator}  {next_indicator}")
}

/// Checks if a source line number matches the target line.
fn is_target_line(source_line: u32, target_line: Option<u32>) -> bool {
    target_line.is_some_and(|t| source_line == t)
}

/// Parameters for rendering a single visual line.
struct VisualLineParams<'a> {
    marker: &'a str,
    source_line_num: u32,
    content: &'a str,
    is_first: bool,
}

/// Context for rendering source lines with line numbers.
struct LineRenderContext {
    /// Width of the line number column.
    line_num_width: usize,
    /// Maximum width for wrapping content.
    max_width: usize,
    /// Optional target line to highlight.
    target_line: Option<u32>,
}

impl LineRenderContext {
    /// Renders a source line (potentially wrapped) to the output.
    fn render_source_line(&self, output: &mut String, source_line: &str, source_line_num: u32) {
        let is_target = is_target_line(source_line_num, self.target_line);
        let marker = if is_target { ">" } else { " " };

        let wrapped = wrap_code_block(source_line, self.max_width);
        let visual_lines: Vec<&str> = wrapped.lines().collect();

        if visual_lines.is_empty() {
            let params = VisualLineParams {
                marker,
                source_line_num,
                content: "",
                is_first: true,
            };
            self.write_line(output, &params);
        } else {
            for (vi, visual_line) in visual_lines.iter().enumerate() {
                let params = VisualLineParams {
                    marker,
                    source_line_num,
                    content: visual_line,
                    is_first: vi == 0,
                };
                self.write_line(output, &params);
            }
        }
    }

    /// Writes a single visual line to output.
    fn write_line(&self, output: &mut String, params: &VisualLineParams<'_>) {
        let line_num_width = self.line_num_width;
        let VisualLineParams {
            marker,
            source_line_num,
            content,
            is_first,
        } = params;
        // Ignoring error as writing to String cannot fail
        #[expect(
            clippy::let_underscore_must_use,
            reason = "Writing to String cannot fail"
        )]
        let _ = if *is_first {
            writeln!(
                output,
                "{marker}{source_line_num:>line_num_width$} | {content}"
            )
        } else {
            writeln!(output, "{marker}{:>line_num_width$} | {content}", "..")
        };
    }
}

fn render_file_content(state: &TimeTravelState, max_width: usize) -> String {
    let Some(content) = state.snapshot().file_content() else {
        return "(File content not available)\n".to_owned();
    };

    if content.is_empty() {
        return "(Empty file)\n".to_owned();
    }

    let source_lines: Vec<&str> = content.lines().collect();
    let ctx = LineRenderContext {
        line_num_width: source_lines.len().to_string().len().max(3),
        max_width,
        target_line: state.original_line(),
    };

    let mut output = String::new();

    // Iterate over source lines, wrapping each individually to preserve
    // source line numbers through wrapping
    for (i, source_line) in source_lines.iter().enumerate() {
        let source_line_num = u32::try_from(i + 1).unwrap_or(u32::MAX);
        ctx.render_source_line(&mut output, source_line, source_line_num);
    }

    output
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use rstest::{fixture, rstest};

    use super::*;
    use crate::local::{CommitMetadata, CommitSnapshot, LineMappingVerification};
    use crate::tui::state::TimeTravelInitParams;

    /// Creates a standard context with common settings.
    fn create_test_context(state: &TimeTravelState) -> TimeTravelViewContext<'_> {
        TimeTravelViewContext {
            state: Some(state),
            max_width: 80,
            max_height: 0,
        }
    }

    /// Creates context and renders the view.
    fn render_view_with_state(state: &TimeTravelState) -> String {
        let ctx = create_test_context(state);
        TimeTravelViewComponent::view(&ctx)
    }

    #[fixture]
    fn sample_state() -> TimeTravelState {
        let metadata = CommitMetadata::new(
            "abc1234567890".to_owned(),
            "Fix login validation".to_owned(),
            "Alice".to_owned(),
            Utc::now(),
        );
        let snapshot = CommitSnapshot::with_file_content(
            metadata,
            "src/auth.rs".to_owned(),
            "fn login() {\n    validate();\n}\n".to_owned(),
        );

        TimeTravelState::new(TimeTravelInitParams {
            snapshot,
            file_path: "src/auth.rs".to_owned(),
            original_line: Some(2),
            line_mapping: Some(LineMappingVerification::exact(2)),
            commit_history: vec!["abc1234567890".to_owned(), "def5678901234".to_owned()],
            current_index: 0,
        })
    }

    #[rstest]
    fn view_shows_commit_header(sample_state: TimeTravelState) {
        let output = render_view_with_state(&sample_state);

        assert!(output.contains("abc1234"));
        assert!(output.contains("Fix login validation"));
    }

    #[rstest]
    fn view_shows_file_path(sample_state: TimeTravelState) {
        let output = render_view_with_state(&sample_state);

        assert!(output.contains("src/auth.rs"));
    }

    #[rstest]
    fn view_shows_line_mapping(sample_state: TimeTravelState) {
        let output = render_view_with_state(&sample_state);

        assert!(output.contains("exact match"));
    }

    #[rstest]
    fn view_shows_navigation(sample_state: TimeTravelState) {
        let output = render_view_with_state(&sample_state);

        assert!(output.contains("Commit 1/2"));
        assert!(output.contains("[h] Previous"));
    }

    #[rstest]
    fn view_shows_file_content(sample_state: TimeTravelState) {
        let output = render_view_with_state(&sample_state);

        assert!(output.contains("fn login()"));
        assert!(output.contains("validate()"));
    }

    #[rstest]
    fn view_highlights_target_line(sample_state: TimeTravelState) {
        let output = render_view_with_state(&sample_state);

        // Line 2 should have the > marker
        assert!(output.contains(">  2 |"));
    }

    #[test]
    fn view_shows_placeholder_when_no_state() {
        let ctx = TimeTravelViewContext {
            state: None,
            max_width: 80,
            max_height: 0,
        };

        let output = TimeTravelViewComponent::view(&ctx);

        assert!(output.contains(NO_STATE_PLACEHOLDER));
    }

    #[test]
    fn view_shows_loading() {
        let state = TimeTravelState::loading("src/main.rs".to_owned(), Some(10));
        let output = render_view_with_state(&state);

        assert!(output.contains(LOADING_PLACEHOLDER));
    }

    #[test]
    fn view_shows_error() {
        let state = TimeTravelState::error("Commit not found".to_owned(), "src/main.rs".to_owned());
        let output = render_view_with_state(&state);

        assert!(output.contains("Error: Commit not found"));
    }

    #[test]
    fn view_preserves_source_line_numbers_through_wrapping() {
        // Create content where line 2 is very long and will wrap
        let long_line = "x".repeat(100);
        let content = format!("short\n{long_line}\nthird");

        let metadata = CommitMetadata::new(
            "abc1234567890".to_owned(),
            "Test".to_owned(),
            "Alice".to_owned(),
            Utc::now(),
        );
        let snapshot =
            CommitSnapshot::with_file_content(metadata, "test.rs".to_owned(), content.clone());

        // Target line 3 (the "third" line)
        let state = TimeTravelState::new(TimeTravelInitParams {
            snapshot,
            file_path: "test.rs".to_owned(),
            original_line: Some(3),
            line_mapping: None,
            commit_history: vec!["abc1234567890".to_owned()],
            current_index: 0,
        });

        // Use narrow width to force wrapping
        let ctx = TimeTravelViewContext {
            state: Some(&state),
            max_width: 40,
            max_height: 0,
        };
        let output = TimeTravelViewComponent::view(&ctx);

        // Line 3 should be highlighted even though line 2 wraps into multiple visual lines
        assert!(
            output.contains(">  3 | third"),
            "Line 3 should be highlighted. Output:\n{output}"
        );

        // Line 2 (wrapped) should NOT have the > marker
        assert!(
            !output.contains(">  2 |"),
            "Line 2 should not be highlighted. Output:\n{output}"
        );

        // Continuation lines should show dots
        assert!(
            output.contains(".. |"),
            "Wrapped continuation should show dots. Output:\n{output}"
        );
    }
}
