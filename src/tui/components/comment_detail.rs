//! Comment detail component for rendering a selected review comment.
//!
//! This component displays a single review comment's metadata, body text,
//! and inline code context with optional syntax highlighting. Code context
//! is extracted from the comment's `diff_hunk` field and wrapped to a
//! maximum width (typically 80 columns or the terminal width if narrower).

use crate::ai::{CommentRewriteMode, SideBySideLine};
use crate::github::models::ReviewComment;

use super::code_highlight::CodeHighlighter;
use super::text_truncate::truncate_to_height;
use super::text_wrap::wrap_text;

/// Placeholder message when no code context is available.
const NO_CONTEXT_PLACEHOLDER: &str = "(No code context available)";

/// Placeholder message when no comment is selected.
const NO_SELECTION_PLACEHOLDER: &str = "(No comment selected)";

/// Context for rendering the comment detail view.
///
/// Bundles the data needed to render a comment detail pane without
/// requiring per-frame allocations.
#[derive(Debug, Clone)]
pub struct CommentDetailViewContext<'a> {
    /// The selected review comment to display, if any.
    pub selected_comment: Option<&'a ReviewComment>,
    /// Maximum width for code block wrapping (typically 80).
    pub max_width: usize,
    /// Maximum height in lines for the detail pane (0 = unlimited).
    pub max_height: usize,
    /// Inline reply draft details for the selected comment, if active.
    pub reply_draft: Option<ReplyDraftRenderContext<'a>>,
    /// Pending AI rewrite preview details for the active reply draft, if any.
    pub reply_draft_ai_preview: Option<ReplyDraftAiPreviewRenderContext<'a>>,
}

/// Render-only reply-draft context for the comment detail view.
#[derive(Debug, Clone)]
pub struct ReplyDraftRenderContext<'a> {
    /// Current reply draft text.
    pub text: &'a str,
    /// Current character count of the draft text.
    pub char_count: usize,
    /// Maximum configured character count.
    pub max_length: usize,
    /// Whether the draft has been marked ready to send.
    pub ready_to_send: bool,
    /// Provenance label for the current draft text.
    pub origin_label: Option<&'a str>,
}

/// Render-only AI preview context for reply-draft rewrite suggestions.
#[derive(Debug, Clone)]
pub struct ReplyDraftAiPreviewRenderContext<'a> {
    /// Rewrite mode used for this candidate.
    pub mode: CommentRewriteMode,
    /// Provenance label to show with the candidate output.
    pub origin_label: &'a str,
    /// Side-by-side preview lines (original and candidate).
    pub lines: &'a [SideBySideLine],
    /// Whether the candidate differs from the original text.
    pub has_changes: bool,
}

/// Component for displaying a single review comment with code context.
///
/// Renders the comment's metadata (author, file, line), body text, and
/// inline code context from the `diff_hunk` field with syntax highlighting.
#[derive(Debug)]
pub struct CommentDetailComponent {
    /// Syntax highlighter for code blocks.
    highlighter: CodeHighlighter,
}

impl Default for CommentDetailComponent {
    fn default() -> Self {
        Self::new()
    }
}

impl CommentDetailComponent {
    /// Creates a new comment detail component.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            highlighter: CodeHighlighter::new(),
        }
    }

    /// Renders the comment detail as a string.
    ///
    /// Returns a formatted string containing:
    /// - A separator line
    /// - Comment header (author, file, line number)
    /// - Comment body text
    /// - Code context with syntax highlighting (if available)
    ///
    /// If no comment is selected, returns a placeholder message.
    /// Output is truncated to `max_height` lines if specified (> 0).
    #[must_use]
    pub fn view(&self, ctx: &CommentDetailViewContext<'_>) -> String {
        let Some(comment) = ctx.selected_comment else {
            return format!("{NO_SELECTION_PLACEHOLDER}\n");
        };

        let mut output = String::new();

        // Separator line
        output.push_str(&Self::render_separator(ctx.max_width));
        output.push('\n');

        // Header: [author] file:line
        output.push_str(&Self::render_header(comment));
        output.push('\n');

        // Body text
        output.push_str(&Self::render_body(comment, ctx.max_width));
        output.push('\n');

        // Code context
        output.push_str(&self.render_code_context(comment, ctx.max_width));

        // Inline reply draft for the selected comment.
        if let Some(reply_draft) = &ctx.reply_draft {
            output.push_str(&Self::render_reply_draft(reply_draft, ctx.max_width));
        }
        if let Some(preview) = &ctx.reply_draft_ai_preview {
            output.push_str(&Self::render_ai_preview(preview, ctx.max_width));
        }

        // Truncate to max_height if specified
        if ctx.max_height > 0 {
            truncate_to_height(&mut output, ctx.max_height);
        }

        output
    }

    /// Renders a horizontal separator line.
    ///
    /// Trusts the caller to provide an appropriate width (already clamped
    /// to 80 or terminal width by the caller).
    fn render_separator(width: usize) -> String {
        "\u{2500}".repeat(width)
    }

    /// Renders the comment header with author, file path, and line number.
    fn render_header(comment: &ReviewComment) -> String {
        let author = comment.author.as_deref().unwrap_or("unknown");
        let file = comment.file_path.as_deref().unwrap_or("(no file)");
        let line_suffix = comment
            .line_number
            .map_or_else(String::new, |n| format!(":{n}"));

        format!("[{author}] {file}{line_suffix}")
    }

    /// Renders the comment body text, wrapped to max width.
    fn render_body(comment: &ReviewComment, max_width: usize) -> String {
        let body = comment.body.as_deref().unwrap_or("(no comment text)");
        wrap_text(body, max_width)
    }

    /// Renders the code context with syntax highlighting.
    ///
    /// Uses the comment's `diff_hunk` field as the code source and
    /// attempts to highlight based on the file extension. Falls back
    /// to plain text if highlighting fails.
    fn render_code_context(&self, comment: &ReviewComment, max_width: usize) -> String {
        let Some(diff_hunk) = comment.diff_hunk.as_deref() else {
            return format!("{NO_CONTEXT_PLACEHOLDER}\n");
        };

        if diff_hunk.trim().is_empty() {
            return format!("{NO_CONTEXT_PLACEHOLDER}\n");
        }

        let mut output = String::from("\n");
        output.push_str(&self.highlighter.highlight_or_plain(
            diff_hunk,
            comment.file_path.as_deref(),
            max_width,
        ));
        if !output.ends_with('\n') {
            output.push('\n');
        }

        output
    }

    /// Renders inline reply-draft content and length metadata.
    fn render_reply_draft(reply_draft: &ReplyDraftRenderContext<'_>, max_width: usize) -> String {
        let mut output = String::from("\nReply draft:\n");

        if reply_draft.text.is_empty() {
            output.push_str("(empty)\n");
        } else {
            output.push_str(&wrap_text(reply_draft.text, max_width));
            output.push('\n');
        }
        if let Some(origin_label) = reply_draft.origin_label {
            output.push_str("Origin: ");
            output.push_str(origin_label);
            output.push('\n');
        }

        let readiness_suffix = if reply_draft.ready_to_send {
            " (ready to send)"
        } else {
            ""
        };
        let char_count = reply_draft.text.chars().count();
        debug_assert_eq!(reply_draft.char_count, char_count);
        output.push_str("Length: ");
        output.push_str(&char_count.to_string());
        output.push('/');
        output.push_str(&reply_draft.max_length.to_string());
        output.push_str(readiness_suffix);
        output.push('\n');
        output.push_str("Templates: 1-9  E:expand  W:reword  Enter:ready  Esc:cancel\n");
        output
    }

    fn render_ai_preview(
        preview: &ReplyDraftAiPreviewRenderContext<'_>,
        max_width: usize,
    ) -> String {
        let mut output = format!("\nAI rewrite preview ({}):\n", preview.mode.label());
        output.push_str("Origin: ");
        output.push_str(preview.origin_label);
        output.push('\n');
        output.push_str("Changed: ");
        output.push_str(if preview.has_changes { "yes" } else { "no" });
        output.push('\n');
        output.push_str("Original || Candidate\n");

        for (index, line) in preview.lines.iter().enumerate() {
            let row = format!("{:>3}: {} || {}", index + 1, line.original, line.candidate);
            output.push_str(&wrap_text(row.as_str(), max_width));
            output.push('\n');
        }

        output.push_str("Apply: Y  Discard: N\n");
        output
    }
}

#[cfg(test)]
#[path = "comment_detail_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "comment_detail_ai_preview_tests.rs"]
mod ai_preview_tests;
