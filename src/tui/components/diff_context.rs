//! Full-screen diff context component.
//!
//! Renders a single diff hunk in a full-screen view with syntax highlighting
//! and a header that includes the file path and hunk position.

use crate::tui::state::{DiffHunk, RenderedDiffHunk};

use super::code_highlight::CodeHighlighter;
use super::text_truncate::truncate_to_height;

/// Placeholder shown when no diff hunks are available.
const NO_CONTEXT_PLACEHOLDER: &str = "(No diff context available for this comment)";

/// Context for rendering the full-screen diff view.
#[derive(Debug, Clone)]
pub(crate) struct DiffContextViewContext<'a> {
    /// Rendered diff hunks to display.
    pub hunks: &'a [RenderedDiffHunk],
    /// Current hunk index.
    pub current_index: usize,
    /// Maximum height in lines (0 = unlimited).
    pub max_height: usize,
}

/// Component responsible for rendering full-screen diff context.
#[derive(Debug)]
pub(crate) struct DiffContextComponent {
    highlighter: CodeHighlighter,
}

impl Default for DiffContextComponent {
    fn default() -> Self {
        Self::new()
    }
}

impl DiffContextComponent {
    /// Creates a new diff context component.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// use frankie::tui::components::DiffContextComponent;
    ///
    /// let component = DiffContextComponent::new();
    /// let _ = component;
    /// ```
    #[must_use]
    pub const fn new() -> Self {
        Self {
            highlighter: CodeHighlighter::new(),
        }
    }

    /// Pre-renders diff hunks with syntax highlighting and wrapping.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// use frankie::tui::components::DiffContextComponent;
    /// use frankie::tui::state::DiffHunk;
    ///
    /// let component = DiffContextComponent::new();
    /// let hunks = vec![DiffHunk {
    ///     file_path: Some("src/main.rs".to_owned()),
    ///     line_number: Some(1),
    ///     text: "@@ -1 +1 @@\n+fn main() {}".to_owned(),
    /// }];
    ///
    /// let rendered = component.render_hunks(&hunks, 80);
    /// assert_eq!(rendered.len(), 1);
    /// ```
    #[must_use]
    pub(crate) fn render_hunks(
        &self,
        hunks: &[DiffHunk],
        max_width: usize,
    ) -> Vec<RenderedDiffHunk> {
        hunks
            .iter()
            .map(|hunk| RenderedDiffHunk {
                hunk: hunk.clone(),
                rendered: ensure_trailing_newline(self.highlighter.highlight_or_plain(
                    &hunk.text,
                    hunk.file_path.as_deref(),
                    max_width,
                )),
            })
            .collect()
    }

    /// Renders the current diff hunk view as a string.
    ///
    /// The output includes a header with file metadata and the hunk body.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// use frankie::tui::components::{DiffContextComponent, DiffContextViewContext};
    /// let component = DiffContextComponent::new();
    /// let ctx = DiffContextViewContext {
    ///     hunks: &[],
    ///     current_index: 0,
    ///     max_height: 0,
    /// };
    /// let output = DiffContextComponent::view(&ctx);
    /// assert!(output.contains("No diff context"));
    /// ```
    #[must_use]
    pub(crate) fn view(ctx: &DiffContextViewContext<'_>) -> String {
        if ctx.hunks.is_empty() {
            return format!("{NO_CONTEXT_PLACEHOLDER}\n");
        }

        let total = ctx.hunks.len();
        let current_index = clamp_index(ctx.current_index, total);
        let Some(current) = ctx.hunks.get(current_index) else {
            return format!("{NO_CONTEXT_PLACEHOLDER}\n");
        };

        let header = render_header(&current.hunk, current_index, total);
        let mut body = current.rendered.clone();

        if ctx.max_height > 0 {
            let body_height = ctx.max_height.saturating_sub(1);
            if body_height == 0 {
                return format!("{header}\n");
            }
            truncate_to_height(&mut body, body_height);
        }

        let mut output = String::new();
        output.push_str(&header);
        output.push('\n');
        output.push_str(&body);
        output
    }
}

fn clamp_index(index: usize, len: usize) -> usize {
    if len == 0 {
        0
    } else {
        index.min(len.saturating_sub(1))
    }
}

fn render_header(hunk: &DiffHunk, current_index: usize, total: usize) -> String {
    let file = hunk.file_path.as_deref().unwrap_or("(no file)");
    let line_suffix = hunk
        .line_number
        .map_or_else(String::new, |line| format!(":{line}"));

    format!(
        "File: {file}{line_suffix}  Hunk {}/{}",
        current_index + 1,
        total
    )
}

fn ensure_trailing_newline(mut value: String) -> String {
    if !value.ends_with('\n') {
        value.push('\n');
    }
    value
}

#[cfg(test)]
mod tests {
    use rstest::{fixture, rstest};

    use super::*;
    use crate::tui::components::test_utils::strip_ansi_codes;

    #[fixture]
    fn sample_hunk() -> DiffHunk {
        DiffHunk {
            file_path: Some("src/main.rs".to_owned()),
            line_number: Some(1),
            text: "@@ -1 +1 @@\n+fn main() {}".to_owned(),
        }
    }

    #[rstest]
    fn render_hunks_returns_rendered_output(sample_hunk: DiffHunk) {
        let component = DiffContextComponent::new();
        let rendered = component.render_hunks(&[sample_hunk], 80);

        assert_eq!(rendered.len(), 1);
        let first = rendered.first().expect("rendered hunks should be present");
        let stripped = strip_ansi_codes(&first.rendered);
        assert!(stripped.contains("fn main"));
    }

    #[rstest]
    fn view_includes_header(sample_hunk: DiffHunk) {
        let component = DiffContextComponent::new();
        let rendered = component.render_hunks(&[sample_hunk], 80);
        let ctx = DiffContextViewContext {
            hunks: &rendered,
            current_index: 0,
            max_height: 0,
        };

        let output = DiffContextComponent::view(&ctx);

        assert!(output.contains("File: src/main.rs:1"));
        assert!(output.contains("Hunk 1/1"));
    }

    #[test]
    fn view_shows_placeholder_when_empty() {
        let ctx = DiffContextViewContext {
            hunks: &[],
            current_index: 0,
            max_height: 0,
        };

        let output = DiffContextComponent::view(&ctx);

        assert!(output.contains(NO_CONTEXT_PLACEHOLDER));
    }

    #[rstest]
    fn view_keeps_header_when_height_is_one(sample_hunk: DiffHunk) {
        let component = DiffContextComponent::new();
        let rendered = component.render_hunks(&[sample_hunk], 80);
        let ctx = DiffContextViewContext {
            hunks: &rendered,
            current_index: 0,
            max_height: 1,
        };

        let output = DiffContextComponent::view(&ctx);
        let stripped = strip_ansi_codes(&output);

        assert!(stripped.contains("File: src/main.rs:1"));
        assert!(!stripped.contains("fn main"));
    }
}
