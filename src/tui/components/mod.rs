//! UI components for the review listing TUI.
//!
//! This module provides reusable UI components following the bubbletea-rs
//! Model-View pattern. Each component manages its own state and rendering.

mod code_highlight;
mod comment_detail;
mod review_list;
pub mod test_utils;
mod text_wrap;

pub use code_highlight::CodeHighlighter;
pub use comment_detail::{CommentDetailComponent, CommentDetailViewContext};
pub use review_list::{ReviewListComponent, ReviewListViewContext};
