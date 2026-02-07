//! Structured comment export functionality.
//!
//! This module re-exports the export types and functions from the library
//! crate for use by the CLI. The actual implementations live in the library
//! to allow sharing with integration tests.

pub use frankie::{
    ExportFormat, ExportedComment, sort_comments, write_jsonl, write_markdown, write_template,
};
