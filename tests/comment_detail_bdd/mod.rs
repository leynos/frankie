//! Support modules for comment detail BDD tests.

pub(crate) mod state;
pub(crate) mod truncation_steps;

pub(crate) use state::DetailState;
pub(crate) use state::ReviewCommentBuilder;

// Re-export truncation step functions for use in the main test file.
// Note: Scenario bindings remain in the main test file as they reference
// the fixture defined there.
pub(crate) use truncation_steps::given_comment_producing_many_lines;
pub(crate) use truncation_steps::given_comment_with_blank_lines;
pub(crate) use truncation_steps::then_blank_lines_preserved;
pub(crate) use truncation_steps::then_last_line_is_ellipsis;
pub(crate) use truncation_steps::then_output_has_max_lines;
pub(crate) use truncation_steps::when_view_rendered_with_max_height;
