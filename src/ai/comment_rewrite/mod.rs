//! AI-powered comment rewrite APIs shared by TUI and CLI adapters.

mod model;
mod openai;
mod preview;
mod service;

pub use model::{
    CommentRewriteContext, CommentRewriteFallback, CommentRewriteGenerated, CommentRewriteMode,
    CommentRewriteModeParseError, CommentRewriteOutcome, CommentRewriteRequest,
};
pub use openai::{OpenAiCommentRewriteConfig, OpenAiCommentRewriteService};
pub use preview::{SideBySideDiffPreview, SideBySideLine, build_side_by_side_diff_preview};
pub use service::{CommentRewriteService, rewrite_with_fallback};
