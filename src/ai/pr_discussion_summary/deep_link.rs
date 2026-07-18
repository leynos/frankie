//! Presentation projection of review-view references as Frankie deep links.
//!
//! This module is an adapter/presentation concern, not the canonical shared
//! contract. The canonical contract is the serde representation of
//! [`ReviewViewRef`].

use std::fmt;

use super::model::ReviewViewRef;

/// Presentation wrapper rendering a [`ReviewViewRef`] as a Frankie deep link.
///
/// Use this wrapper when an adapter needs the compatibility
/// `frankie://review-comment/<id>?view=<view>` token. Keep URI rendering here
/// rather than on [`ReviewViewRef`] so the shared DTO remains host-neutral.
///
/// # Examples
///
/// ```rust
/// use frankie::ai::{FrankieDeepLink, ReviewViewRef};
///
/// let view_ref = ReviewViewRef::comment_detail(42_u64.into());
///
/// assert_eq!(
///     FrankieDeepLink::new(&view_ref).to_string(),
///     "frankie://review-comment/42?view=detail"
/// );
/// ```
pub struct FrankieDeepLink<'a>(&'a ReviewViewRef);

impl<'a> FrankieDeepLink<'a> {
    /// Creates a presentation wrapper for the provided review-view reference.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use frankie::ai::{FrankieDeepLink, ReviewViewRef};
    ///
    /// let view_ref = ReviewViewRef::comment_detail(7_u64.into());
    /// let deep_link = FrankieDeepLink::new(&view_ref);
    ///
    /// assert_eq!(
    ///     deep_link.to_string(),
    ///     "frankie://review-comment/7?view=detail"
    /// );
    /// ```
    #[must_use]
    pub const fn new(view_ref: &'a ReviewViewRef) -> Self {
        Self(view_ref)
    }
}

impl fmt::Display for FrankieDeepLink<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        // The `//` authority form is intentionally preserved for compatibility
        // with existing CLI/TUI output, despite not naming a true authority.
        write!(
            formatter,
            "frankie://review-comment/{}?view={}",
            self.0.comment_id.as_u64(),
            self.0.view.label()
        )
    }
}
