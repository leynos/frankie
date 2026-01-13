//! UI components for the review listing TUI.
//!
//! This module provides reusable UI components following the bubbletea-rs
//! Model-View pattern. Each component manages its own state and rendering.

pub mod code_highlight;
mod comment_detail;
mod review_list;

pub use code_highlight::{CodeHighlighter, HighlightError};
pub use comment_detail::{CommentDetailComponent, CommentDetailViewContext};
pub use review_list::{ReviewListComponent, ReviewListViewContext};
