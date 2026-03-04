//! Shared time utilities.

use std::time::{SystemTime, UNIX_EPOCH};

/// Returns the current Unix timestamp in seconds as `i64`.
#[must_use]
pub fn unix_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        .try_into()
        .unwrap_or(i64::MAX)
}
