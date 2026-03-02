//! Automated resolution verification via diff replay.
//!
//! This module provides reusable library APIs for verifying whether a review
//! comment appears to have been addressed in code by replaying diffs between
//! the comment's commit and a target commit (typically the repository `HEAD`).
//!
//! Verification is intentionally conservative and deterministic: it does not
//! attempt to interpret the intent of the comment. Instead, it classifies a
//! comment as verified when the referenced line has been removed or its content
//! has changed between the two commits.

mod model;
mod service;

pub use model::{
    CommentVerificationEvidence, CommentVerificationEvidenceKind, CommentVerificationResult,
    CommentVerificationStatus,
};
pub use service::{DiffReplayResolutionVerifier, ResolutionVerificationService};
