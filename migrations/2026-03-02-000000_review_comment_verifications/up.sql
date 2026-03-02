-- Cache table for automated resolution verification results.
--
-- Stores the latest verification status for a GitHub review comment when
-- verified against a specific target commit SHA (typically HEAD).

CREATE TABLE review_comment_verifications (
    id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    github_comment_id INTEGER NOT NULL,
    target_sha TEXT NOT NULL,
    status TEXT NOT NULL CHECK(status IN ('verified', 'unverified')),
    evidence_kind TEXT NOT NULL,
    evidence_message TEXT,
    verified_at_unix INTEGER NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(github_comment_id, target_sha)
);

CREATE INDEX idx_review_comment_verifications_target
    ON review_comment_verifications(target_sha);

