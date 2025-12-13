//! Rate limit information from GitHub API responses.
//!
//! This module provides the `RateLimitInfo` type for capturing rate limit
//! headers returned by the GitHub API. Rate limit information helps callers
//! implement backoff strategies and avoid exhausting their API quota.

use std::time::{SystemTime, UNIX_EPOCH};

/// Rate limit information extracted from GitHub API response headers.
///
/// GitHub includes rate limit headers (`X-RateLimit-Limit`, `X-RateLimit-Remaining`,
/// `X-RateLimit-Reset`) in API responses. This struct captures those values for
/// inspection by callers.
///
/// # Example
///
/// ```
/// use frankie::github::rate_limit::RateLimitInfo;
///
/// let info = RateLimitInfo::new(5000, 4999, 1700000000);
/// assert!(!info.is_exhausted());
/// assert_eq!(info.remaining(), 4999);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RateLimitInfo {
    /// Maximum requests allowed in the current window.
    limit: u32,
    /// Remaining requests in the current window.
    remaining: u32,
    /// Unix timestamp when the rate limit resets.
    reset_at: u64,
}

impl RateLimitInfo {
    /// Creates a new rate limit info instance.
    #[must_use]
    pub const fn new(limit: u32, remaining: u32, reset_at: u64) -> Self {
        Self {
            limit,
            remaining,
            reset_at,
        }
    }

    /// Returns the maximum requests allowed in the current window.
    #[must_use]
    pub const fn limit(&self) -> u32 {
        self.limit
    }

    /// Returns the remaining requests in the current window.
    #[must_use]
    pub const fn remaining(&self) -> u32 {
        self.remaining
    }

    /// Returns the Unix timestamp when the rate limit resets.
    #[must_use]
    pub const fn reset_at(&self) -> u64 {
        self.reset_at
    }

    /// Returns true if the rate limit has been exhausted.
    #[must_use]
    pub const fn is_exhausted(&self) -> bool {
        self.remaining == 0
    }

    /// Calculates seconds until the rate limit resets.
    ///
    /// Returns 0 if the reset time has already passed or if the system time
    /// cannot be determined.
    #[must_use]
    pub fn seconds_until_reset(&self) -> u64 {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_secs())
            .unwrap_or(0);

        self.reset_at.saturating_sub(now)
    }
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::RateLimitInfo;

    #[test]
    fn seconds_until_reset_returns_zero_when_reset_has_passed() {
        let info = RateLimitInfo::new(5000, 0, 0);
        assert_eq!(info.seconds_until_reset(), 0);
    }

    #[test]
    fn seconds_until_reset_returns_positive_for_future_reset() {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be available")
            .as_secs();
        let info = RateLimitInfo::new(5000, 0, now + 60);

        let seconds = info.seconds_until_reset();
        assert!(
            (1..=60).contains(&seconds),
            "expected 1..=60 seconds until reset, got {seconds}"
        );
    }
}
