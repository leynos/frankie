//! Shared data helpers for the repository listing BDD tests.

use serde_json::json;

use super::domain::{PageNumber, PullRequestCount};

pub(crate) const EXPECTED_RATE_LIMIT_RESET_AT: u64 = 1_700_000_000;

pub(crate) fn generate_pr_list(
    count: PullRequestCount,
    page: PageNumber,
    per_page: PullRequestCount,
) -> Vec<serde_json::Value> {
    let start = (page.value() - 1) * per_page.value();
    (0..count.value())
        .map(|i| {
            let pr_number = start + i + 1;
            json!({
                "number": pr_number,
                "title": format!("PR #{pr_number}"),
                "state": "open",
                "user": { "login": "contributor" },
                "created_at": "2024-01-01T00:00:00Z",
                "updated_at": "2024-01-01T00:00:00Z"
            })
        })
        .collect()
}
