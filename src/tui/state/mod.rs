//! State management for the review listing TUI.
//!
//! This module provides the core state types for managing filter criteria
//! and cursor position in the review list.

mod filter_state;

pub use filter_state::{FilterState, ReviewFilter};
