//! CLI operation mode handlers.
//!
//! This module contains the implementations for different operation modes:
//! - [`interactive`]: Local repository discovery and listing
//! - [`migrations`]: Database schema migrations
//! - [`repository_listing`]: List PRs for a specified repository
//! - [`single_pr`]: Load details for a single pull request
//!
//! Output formatting utilities are in [`output`].

pub mod interactive;
pub mod migrations;
pub mod output;
pub mod repository_listing;
pub mod single_pr;
