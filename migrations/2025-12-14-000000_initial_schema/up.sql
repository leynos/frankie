PRAGMA foreign_keys = ON;

CREATE TABLE repositories (
    id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    owner TEXT NOT NULL,
    name TEXT NOT NULL,
    remote_url TEXT NOT NULL,
    default_branch TEXT,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    last_synced TIMESTAMP
);

CREATE UNIQUE INDEX idx_repositories_owner_name
    ON repositories(owner, name);

CREATE TABLE pull_requests (
    id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    repository_id INTEGER NOT NULL REFERENCES repositories(id) ON DELETE CASCADE,
    pr_number INTEGER NOT NULL,
    title TEXT NOT NULL,
    body TEXT,
    state TEXT NOT NULL,
    head_sha TEXT,
    base_sha TEXT,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    last_synced TIMESTAMP,
    UNIQUE(repository_id, pr_number)
);

CREATE TABLE review_comments (
    id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    pull_request_id INTEGER NOT NULL REFERENCES pull_requests(id) ON DELETE CASCADE,
    github_comment_id INTEGER NOT NULL,
    body TEXT NOT NULL,
    file_path TEXT,
    line_number INTEGER,
    original_line_number INTEGER,
    diff_hunk TEXT,
    resolution_status TEXT NOT NULL DEFAULT 'unresolved',
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(github_comment_id)
);

CREATE INDEX idx_review_comments_pr_status
    ON review_comments(pull_request_id, resolution_status);

CREATE TABLE sync_checkpoints (
    id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    repository_id INTEGER NOT NULL REFERENCES repositories(id) ON DELETE CASCADE,
    resource TEXT NOT NULL,
    checkpoint TEXT,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(repository_id, resource)
);
