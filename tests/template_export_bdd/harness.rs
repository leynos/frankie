//! Test data generators for template export BDD tests.

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

/// Generates a reply comment for testing status placeholder.
pub(crate) fn generate_reply_comment() -> serde_json::Value {
    json!([
        {
            "id": 2,
            "body": "This is a reply",
            "user": { "login": "replier" },
            "path": "src/reply.rs",
            "line": 15,
            "original_line": 15,
            "diff_hunk": "@@ -13,3 +13,5 @@",
            "commit_id": "abc0002",
            "in_reply_to_id": 1,
            "created_at": "2025-01-02T10:00:00Z",
            "updated_at": "2025-01-02T11:00:00Z"
        }
    ])
}
