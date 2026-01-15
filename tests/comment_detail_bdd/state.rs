//! Scenario state for comment detail BDD tests.

use frankie::github::models::ReviewComment;
use frankie::tui::app::ReviewApp;
use rstest_bdd::Slot;
use rstest_bdd_macros::ScenarioState;

/// State shared across steps in a comment detail scenario.
#[derive(ScenarioState, Default)]
pub(crate) struct DetailState {
    /// The TUI application model under test.
    pub(crate) app: Slot<ReviewApp>,
    /// The rendered view output.
    pub(crate) rendered_view: Slot<String>,
}

/// Builder for creating test `ReviewComment` instances.
///
/// Provides a fluent API for constructing review comments with only
/// the fields relevant to each test case.
#[derive(Default)]
pub(crate) struct ReviewCommentBuilder {
    id: u64,
    author: Option<String>,
    file_path: Option<String>,
    line_number: Option<u32>,
    body: Option<String>,
    diff_hunk: Option<String>,
}

impl ReviewCommentBuilder {
    /// Creates a new builder with the specified comment ID.
    #[must_use]
    pub(crate) fn new(id: u64) -> Self {
        Self {
            id,
            ..Default::default()
        }
    }

    /// Sets the comment author.
    #[must_use]
    pub(crate) fn author(mut self, author: &str) -> Self {
        self.author = Some(author.to_owned());
        self
    }

    /// Sets the file path for the comment.
    #[must_use]
    pub(crate) fn file_path(mut self, path: &str) -> Self {
        self.file_path = Some(path.to_owned());
        self
    }

    /// Sets the line number for the comment.
    #[must_use]
    pub(crate) const fn line_number(mut self, line: u32) -> Self {
        self.line_number = Some(line);
        self
    }

    /// Sets the comment body text.
    #[must_use]
    pub(crate) fn body(mut self, body: &str) -> Self {
        self.body = Some(body.to_owned());
        self
    }

    /// Sets the diff hunk for inline code context.
    #[must_use]
    pub(crate) fn diff_hunk(mut self, hunk: &str) -> Self {
        self.diff_hunk = Some(hunk.to_owned());
        self
    }

    /// Builds the `ReviewComment` instance.
    #[must_use]
    pub(crate) fn build(self) -> ReviewComment {
        ReviewComment {
            id: self.id,
            body: self.body,
            author: self.author,
            file_path: self.file_path,
            line_number: self.line_number,
            original_line_number: None,
            diff_hunk: self.diff_hunk,
            commit_sha: None,
            in_reply_to_id: None,
            created_at: None,
            updated_at: None,
        }
    }
}
