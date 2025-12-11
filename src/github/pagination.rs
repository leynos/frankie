//! Pagination state and navigation for GitHub API responses.
//!
//! This module provides types for tracking pagination state when listing
//! resources from the GitHub API. The `PageInfo` struct captures the current
//! page position and navigation availability.

/// Current page state for paginated results.
///
/// Tracks the current position within a paginated result set and provides
/// navigation predicates for determining whether additional pages exist.
///
/// # Example
///
/// ```
/// use frankie::github::pagination::PageInfo;
///
/// let info = PageInfo::new(2, 50, Some(5), true, true);
/// assert!(!info.is_first_page());
/// assert!(!info.is_last_page());
/// assert!(info.has_next());
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PageInfo {
    /// Current page number (1-based).
    current_page: u32,
    /// Items per page.
    per_page: u8,
    /// Total number of pages if known.
    total_pages: Option<u32>,
    /// Whether more pages exist after the current one.
    has_next: bool,
    /// Whether pages exist before the current one.
    has_prev: bool,
}

impl PageInfo {
    /// Creates a new page info instance.
    #[must_use]
    #[expect(
        clippy::too_many_arguments,
        reason = "pagination state requires all fields for correct behavior"
    )]
    pub const fn new(
        current_page: u32,
        per_page: u8,
        total_pages: Option<u32>,
        has_next: bool,
        has_prev: bool,
    ) -> Self {
        Self {
            current_page,
            per_page,
            total_pages,
            has_next,
            has_prev,
        }
    }

    /// Returns the current page number (1-based).
    #[must_use]
    pub const fn current_page(&self) -> u32 {
        self.current_page
    }

    /// Returns the number of items per page.
    #[must_use]
    pub const fn per_page(&self) -> u8 {
        self.per_page
    }

    /// Returns the total number of pages if known.
    #[must_use]
    pub const fn total_pages(&self) -> Option<u32> {
        self.total_pages
    }

    /// Returns true if more pages exist after the current one.
    #[must_use]
    pub const fn has_next(&self) -> bool {
        self.has_next
    }

    /// Returns true if pages exist before the current one.
    #[must_use]
    pub const fn has_prev(&self) -> bool {
        self.has_prev
    }

    /// Returns true if this is the first page.
    #[must_use]
    pub const fn is_first_page(&self) -> bool {
        self.current_page == 1
    }

    /// Returns true if this is the last page.
    #[must_use]
    pub const fn is_last_page(&self) -> bool {
        !self.has_next
    }
}

impl Default for PageInfo {
    fn default() -> Self {
        Self::new(1, 30, None, false, false)
    }
}
