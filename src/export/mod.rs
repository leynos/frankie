//! Structured comment export functionality.
//!
//! This module provides utilities for exporting pull request review comments
//! in structured formats (Markdown and JSONL) suitable for downstream
//! processing by artificial intelligence (AI) tools or human review.
//!
//! # Supported Formats
//!
//! - **Markdown**: Human-readable format with syntax-highlighted code blocks
//! - **JSONL**: Machine-readable JSON Lines format (one object per line)
//!
//! # Ordering
//!
//! Comments are exported in stable order: by file path (alphabetical), then
//! line number (ascending), then comment ID (ascending). Comments with missing
//! file paths or line numbers are sorted last.

mod jsonl;
mod markdown;
mod model;
mod ordering;
#[doc(hidden)]
pub mod test_helpers;

pub use jsonl::write_jsonl;
pub use markdown::write_markdown;
pub use model::{ExportFormat, ExportedComment, PrUrl};
pub use ordering::sort_comments;
