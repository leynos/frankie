//! UI components for the review listing TUI.
//!
//! This module provides reusable UI components following the bubbletea-rs
//! Model-View pattern. Each component manages its own state and rendering.

mod code_highlight;
mod comment_detail;
mod review_list;
mod text_wrap;

#[cfg(any(test, feature = "test-support"))]
pub mod test_utils;

pub use code_highlight::CodeHighlighter;
pub use comment_detail::{CommentDetailComponent, CommentDetailViewContext};
pub use review_list::{ReviewListComponent, ReviewListViewContext};
