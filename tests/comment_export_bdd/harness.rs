//! Test data generators for comment export BDD tests.

use serde_json::json;
use std::str::FromStr;

/// Number of review comments for parameterised tests.
#[derive(Debug, Clone, Copy)]
pub(crate) struct CommentCount(u32);

impl CommentCount {
    pub(crate) const fn value(self) -> u32 {
        self.0
    }
}

impl FromStr for CommentCount {
    type Err = std::num::ParseIntError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        s.parse().map(Self)
    }
}

/// Generates a list of mock review comments.
pub(crate) fn generate_review_comments(count: CommentCount) -> serde_json::Value {
    let comments: Vec<serde_json::Value> = (1..=count.value())
        .map(|i| {
            json!({
                "id": i,
                "body": format!("Review comment {i}"),
                "user": { "login": format!("reviewer{i}") },
                "path": format!("src/file{i}.rs"),
                "line": i * 10,
                "original_line": i * 10,
                "diff_hunk": format!("@@ -{},3 +{},5 @@\n let x = {};", i * 10, i * 10, i),
                "commit_id": format!("abc{i:04}"),
                "in_reply_to_id": null,
                "created_at": format!("2025-01-{:02}T10:00:00Z", i.min(28)),
                "updated_at": format!("2025-01-{:02}T11:00:00Z", i.min(28))
            })
        })
        .collect();

    json!(comments)
}

/// Generates review comments in random order for testing stable sorting.
pub(crate) fn generate_ordered_comments() -> serde_json::Value {
    // Create comments that will sort to: src/a.rs:10, src/a.rs:20, src/b.rs:5
    // But provide them in different order to test sorting
    json!([
        {
            "id": 3,
            "body": "Comment on b.rs line 5",
            "user": { "login": "alice" },
            "path": "src/b.rs",
            "line": 5,
            "original_line": 5,
            "diff_hunk": "@@ -3,3 +3,5 @@",
            "commit_id": "abc0003",
            "in_reply_to_id": null,
            "created_at": "2025-01-03T10:00:00Z",
            "updated_at": "2025-01-03T11:00:00Z"
        },
        {
            "id": 1,
            "body": "Comment on a.rs line 10",
            "user": { "login": "bob" },
            "path": "src/a.rs",
            "line": 10,
            "original_line": 10,
            "diff_hunk": "@@ -8,3 +8,5 @@",
            "commit_id": "abc0001",
            "in_reply_to_id": null,
            "created_at": "2025-01-01T10:00:00Z",
            "updated_at": "2025-01-01T11:00:00Z"
        },
        {
            "id": 2,
            "body": "Comment on a.rs line 20",
            "user": { "login": "charlie" },
            "path": "src/a.rs",
            "line": 20,
            "original_line": 20,
            "diff_hunk": "@@ -18,3 +18,5 @@",
            "commit_id": "abc0002",
            "in_reply_to_id": null,
            "created_at": "2025-01-02T10:00:00Z",
            "updated_at": "2025-01-02T11:00:00Z"
        }
    ])
}
