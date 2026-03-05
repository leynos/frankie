//! Shared helpers for verify-resolutions behavioural tests.

use std::error::Error;

use diesel::Connection;
use diesel::QueryableByName;
use diesel::RunQueryDsl;
use diesel::sql_query;
use diesel::sql_types::{BigInt, Text};
use git2::{ErrorCode, Oid, Repository};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use super::runtime::SharedRuntime;

pub fn create_commit(
    repo: &Repository,
    message: &str,
    files: &[(&str, &str)],
) -> Result<Oid, Box<dyn Error>> {
    let sig = repo.signature()?;
    let mut index = repo.index()?;

    let workdir = repo
        .workdir()
        .ok_or("repository has no working directory")?;
    for (path, content) in files {
        let full = workdir.join(path);
        if let Some(parent) = full.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&full, content)?;
        index.add_path(std::path::Path::new(path))?;
    }

    let tree_id = index.write_tree()?;
    let tree = repo.find_tree(tree_id)?;

    let parent: Option<git2::Commit<'_>> = match repo.head() {
        Ok(head_ref) => Some(head_ref.peel_to_commit()?),
        Err(e) if e.code() == ErrorCode::UnbornBranch => None,
        Err(e) => return Err(e.into()),
    };
    let parents: Vec<&git2::Commit<'_>> = parent.iter().collect();

    Ok(repo.commit(Some("HEAD"), &sig, &sig, message, &tree, &parents)?)
}

#[derive(Debug, Clone, Copy)]
pub struct ReviewCommentsMount<'a> {
    pub pr: u64,
    pub comment_id: u64,
    pub commit_id: &'a str,
}

pub fn mount_review_comments(
    runtime: &SharedRuntime,
    server: &MockServer,
    mount: ReviewCommentsMount<'_>,
) {
    let comments_path = format!("/api/v3/repos/owner/repo/pulls/{}/comments", mount.pr);
    let response = ResponseTemplate::new(200).set_body_json(serde_json::json!([
        {
            "id": mount.comment_id,
            "body": "Please update this line",
            "user": { "login": "alice" },
            "path": "src/main.rs",
            "line": 2,
            "original_line": 2,
            "diff_hunk": "@@ -1,3 +1,3 @@",
            "commit_id": mount.commit_id,
            "in_reply_to_id": null,
            "created_at": "2026-03-02T00:00:00Z",
            "updated_at": "2026-03-02T00:00:00Z"
        }
    ]));

    runtime.block_on(
        Mock::given(method("GET"))
            .and(path(comments_path))
            .respond_with(response)
            .mount(server),
    );
}

#[derive(QueryableByName)]
struct CountRow {
    #[diesel(sql_type = BigInt)]
    count: i64,
}

pub fn count_cache_rows(
    database_url: &str,
    comment_id: u64,
    target_sha: &str,
) -> Result<i64, Box<dyn Error>> {
    let mut connection = diesel::sqlite::SqliteConnection::establish(database_url)?;
    let comment_id_i64 = i64::try_from(comment_id)?;
    let row: CountRow = sql_query(
        "SELECT COUNT(*) AS count FROM review_comment_verifications \
         WHERE github_comment_id = ? AND target_sha = ?;",
    )
    .bind::<BigInt, _>(comment_id_i64)
    .bind::<Text, _>(target_sha)
    .get_result(&mut connection)?;
    Ok(row.count)
}
