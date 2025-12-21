-- Add a dedicated cache table for pull request metadata fetched via Octocrab.
--
-- The initial schema focuses on domain entities (repositories, pull requests,
-- comments). This cache table is intentionally minimal and keyed by the API
-- base plus owner/repo/number so the CLI can reuse metadata across sessions
-- without requiring the full repository graph to be populated.

CREATE TABLE pr_metadata_cache (
    id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    api_base TEXT NOT NULL,
    owner TEXT NOT NULL,
    repo TEXT NOT NULL,
    pr_number INTEGER NOT NULL,
    title TEXT,
    state TEXT,
    html_url TEXT,
    author TEXT,
    etag TEXT,
    last_modified TEXT,
    fetched_at_unix INTEGER NOT NULL,
    expires_at_unix INTEGER NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(api_base, owner, repo, pr_number)
);

