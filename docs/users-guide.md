# User guide

This guide explains how to load a GitHub pull request in Frankie and what to
expect from the current intake workflow.

- Frankie reads a pull request URL and a personal access token, then fetches
  the pull request metadata and all issue comments.
- Authentication is performed through Octocrab; HTTP 401 or 403 responses are
  surfaced as a clear authentication failure that includes the GitHub error
  message.
- The CLI summarises the pull request number, author, title, HTML URL and the
  total number of comments.

## Prerequisites

- A GitHub personal access token (classic or fine-grained) with permission to
  read pull requests and comments in the target repository.
- Network access to the GitHub host referenced by the pull request URL.

## Running the CLI

```bash
frankie --pr-url https://github.com/owner/repo/pull/123 --token ghp_example
```

- `--pr-url` (or `-u`) is required. Trailing path segments such as `/files`
  are accepted but ignored during parsing.
- `--token` (or `-t`) is optional when the `GITHUB_TOKEN` environment variable
  is set. An empty token fails fast with a user-readable error.
- Hosts other than `github.com` are treated as GitHub Enterprise servers by
  default; the API base is derived as `https://<host>/api/v3`.

## Expected output

A successful call prints a short summary:

```text
Loaded PR #123 by octocat: Add search
URL: https://github.com/owner/repo/pull/123
Comments: 2
```

Authentication or network failures keep the process exit code at non-zero and
emit a clear error message describing the failing step (e.g. "GitHub rejected
the token: Bad credentials").
