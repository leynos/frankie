# User guide

This guide explains how to use Frankie for GitHub pull request workflows,
including loading individual pull requests and listing PRs for a repository.

## Prerequisites

- A GitHub personal access token (classic or fine-grained) with permission to
  read pull requests and comments in the target repository.
- Network access to the GitHub host referenced by the pull request or
  repository URL.

## Operation modes

Frankie supports five operation modes:

1. **Interactive mode** — Auto-detect repository from local Git directory
2. **Single pull request mode** — Load a specific PR by URL using `--pr-url`
3. **Repository listing mode** — List PRs for a repository using `--owner` and
   `--repo`
4. **Review text user interface (TUI) mode** — Interactive terminal interface
   for navigating and filtering review comments using `--tui` with `--pr-url`
5. **Comment export mode** — Export review comments in structured formats using
   `--export` with `--pr-url`

## Interactive mode (local discovery)

When invoked without `--pr-url`, `--owner`, or `--repo`, Frankie automatically
discovers the GitHub repository from the current directory's Git configuration:

```bash
cd /path/to/the-repo
frankie --token ghp_example
```

Frankie reads the `origin` remote URL and extracts the owner/repository
information. Supported remote URL formats:

- SSH: `git@github.com:owner/repo.git`
- HTTPS: `https://github.com/owner/repo.git`
- GitHub Enterprise: `git@ghe.example.com:org/project.git`

### Discovery output

A successful call prints discovery information:

```text
Discovered repository from local Git: owner/repo
```

### Disabling local discovery

To skip local discovery and require explicit arguments, use
`--no-local-discovery`:

```bash
frankie --no-local-discovery --owner octocat --repo hello-world --token ghp_example
```

### Common discovery errors

- **Not inside a Git repository** — The current directory is not within a Git
  working tree.
- **Repository has no remotes configured** — The Git repository has no remote
  URLs configured.
- **Could not parse remote URL** — The `origin` remote URL is not a valid Git
  remote URL.

## Single pull request mode

Load a specific pull request to view its metadata and comments:

```bash
frankie --pr-url https://github.com/owner/repo/pull/123 --token ghp_example
```

- `--pr-url` (or `-u`) specifies the pull request URL. Trailing path segments
  such as `/files` are accepted but ignored during parsing.
- `--token` (or `-t`) is optional when the `FRANKIE_TOKEN` or `GITHUB_TOKEN`
  environment variable is set. An empty token fails fast with a user-readable
  error.
- Hosts other than `github.com` are treated as GitHub Enterprise servers by
  default; the API base is derived as `https://<host>/api/v3`.

### Expected output

A successful call prints a short summary:

```text
Loaded PR #123 by octocat: Add search
URL: https://github.com/owner/repo/pull/123
Comments: 2
```

## Repository listing mode

List pull requests for a repository with pagination support:

```bash
frankie --owner octocat --repo hello-world --token ghp_example
```

- `--owner` (or `-o`) specifies the repository owner (user or organization).
- `--repo` (or `-r`) specifies the repository name.
- Both `--owner` and `--repo` must be provided together.
- The listing displays up to 50 PRs per page with pagination controls.

### Listing output

A successful call prints a listing summary:

```text
Pull requests for octocat/hello-world:

  #42 [open] Add new feature (@alice)
  #41 [closed] Fix bug in parser (@bob)
  #40 [open] Update documentation (@alice)
  ...

Page 1 of 3 (50 PRs shown)
More pages available.
```

### Pagination

The repository listing displays 50 PRs per page by default. Pagination
information shows the current page, total pages, and whether more pages are
available.

### Rate limiting

Frankie handles GitHub API rate limits gracefully:

- Rate limit errors (HTTP 403 with rate limit message) are surfaced as clear
  error messages rather than panics.
- The error includes information about when the rate limit resets, if
  available.

## Review TUI mode

Launch an interactive terminal interface for navigating and filtering review
comments on a pull request:

```bash
frankie --tui --pr-url https://github.com/owner/repo/pull/123 --token ghp_example
```

The TUI uses bubbletea-rs to provide a keyboard-driven experience for reviewing
comments.

### Keyboard shortcuts

Table: Review list keyboard shortcuts.

| Key         | Action                         |
| ----------- | ------------------------------ |
| `j`, `↓`    | Move cursor down               |
| `k`, `↑`    | Move cursor up                 |
| `PgDn`      | Page down                      |
| `PgUp`      | Page up                        |
| `Home`, `g` | Go to first item               |
| `End`, `G`  | Go to last item                |
| `f`         | Cycle filter (All/Unresolved)  |
| `Esc`       | Clear filter or exit context   |
| `c`         | Open full-screen diff context  |
| `[`         | Previous diff hunk             |
| `]`         | Next diff hunk                 |
| `t`         | Enter time-travel mode         |
| `x`         | Run Codex on filtered comments |
| `r`         | Refresh from GitHub            |
| `?`         | Toggle help overlay            |
| `q`         | Quit                           |

#### Time-travel mode keyboard shortcuts

Table: Time-travel mode keyboard shortcuts.

| Key   | Action                              |
| ----- | ----------------------------------- |
| `h`   | Navigate to previous (older) commit |
| `l`   | Navigate to next (newer) commit     |
| `Esc` | Exit time-travel mode               |
| `q`   | Quit                                |

### Background sync

The TUI automatically refreshes review comments from GitHub every 30 seconds.
During a background sync:

- New comments are added to the list
- Updated comments are refreshed
- Deleted comments are removed
- The current selection is preserved (unless the selected comment was deleted)

A `[Loading…]` indicator appears in the header during sync. Manual refresh with
`r` uses the same incremental sync logic.

### Codex execution from the TUI

Press `x` in the review list to run `codex app-server` using the currently
filtered comments as input. Frankie serializes the filtered comments as JSONL,
starts Codex, and polls the process stream via the app-server JSON-RPC protocol.

During execution, the status bar switches from key hints to live Codex status
updates. Each update is derived from streamed JSONL events (or from malformed
line warnings when an event cannot be parsed).

Each run writes a transcript to:

- `${XDG_STATE_HOME:-$HOME/.local/state}/frankie/codex-transcripts/`

Transcript filenames follow this pattern:

- `<owner>-<repo>-pr-<number>-<utc-yyyymmddThhmmssZ>.jsonl`

If Codex exits with a non-zero status, Frankie shows an explicit TUI error that
includes the exit code (when available) and transcript path to aid diagnosis.

### Filters

The TUI supports filtering review comments by several criteria:

- **All** — Show all review comments
- **Unresolved** — Show only comments that are not replies (root comments)
- **By file** — Show comments on a specific file path
- **By reviewer** — Show comments from a specific author
- **By commit range** — Show comments within a commit range

Filters execute locally without requiring a full reload from GitHub. The cursor
position is preserved when changing filters (clamped to valid range if the
filtered list is shorter).

### TUI display

The TUI displays:

- **Header** — Application name with loading indicator when refreshing
- **Filter bar** — Active filter with count of filtered vs total comments
- **Review list** — Scrollable list with cursor indicator showing author, file,
  line number, and a preview of the comment body
- **Comment detail pane** — Displays the selected comment with full body text
  and inline code context
- **Full-screen diff context** — Dedicated view for navigating between diff
  hunks for the current review list selection
- **Status bar** — Keyboard shortcut hints or error message if present

### Comment detail view

When a comment is selected in the review list, the detail pane displays:

- **Comment header** — Author name, file path, and line number
- **Comment body** — Full text of the review comment
- **Code context** — The diff hunk showing the code being reviewed, with syntax
  highlighting when available

Code context is extracted from the GitHub review comment's `diff_hunk` field
and rendered with syntax highlighting based on the file extension. If the file
type is not recognized or highlighting fails, the code is displayed as plain
text.

Long code lines are wrapped to a maximum of 80 columns (or the terminal width
if narrower) to ensure readability without horizontal scrolling.

### Full-screen diff context

Pressing `c` in the review list opens a full-screen diff context view. The view
shows the current diff hunk with file metadata and allows jumping between hunks
using `[` (previous) and `]` (next). Pressing `Esc` returns to the review list
without losing the current selection.

### Time-travel mode

Time-travel mode displays the exact code state when a review comment was made.
This is useful for understanding what was observed at the time a comment was
left, especially when the code has changed significantly since then.

To enter time-travel mode:

1. Select a review comment in the list
2. Press `t` to enter time-travel mode

Frankie loads the commit snapshot associated with the comment and displays:

- **Commit information** — SHA, message, author, and timestamp
- **File content** — The file as it appeared at that commit
- **Line mapping status** — Whether the commented line still exists and where

Line mapping verification shows one of these statuses:

- `✓` **Exact match** — The line is at the same position in both commits
- `→` **Moved** — The line has moved to a different position (shows offset)
- `✗` **Deleted** — The line no longer exists in the current commit
- `?` **Not found** — Unable to locate the line in the commit

#### Navigating commits

While in time-travel mode, the `h` and `l` keys navigate through the commit
history:

- `h` moves to the previous (older) commit
- `l` moves to the next (newer) commit

The header shows the current position in the commit history (e.g., "Commit 2 of
5").

#### Requirements

Time-travel mode requires:

- A local Git repository (Frankie must be run from within the repository or
  the repository must be discoverable)
- The commit SHA referenced by the comment must exist in the local repository

If these requirements are not met, Frankie displays a clear error message
explaining what is missing.

#### Time-travel errors

- **No local repository** — Displays "No local repository available. Clone the
  repository to use time-travel mode."
- **Commit not found** — Displays "Commit not found in local repository. The
  commit may have been force-pushed away."

## Comment export mode

Export review comments in structured formats for downstream processing by
artificial intelligence (AI) tools or human review:

```bash
frankie --pr-url https://github.com/owner/repo/pull/123 --export markdown
```

### Export formats

Frankie supports three export formats:

- **Markdown** (`--export markdown`) — Human-readable format with code blocks
  and syntax highlighting hints
- **JSONL** (`--export jsonl`) — Machine-readable format with one JSON object
  per line
- **Template** (`--export template --template <file>`) — Custom
  Jinja2-compatible template format; requires `--template` with the path to a
  Jinja2 template file

### Output destination

By default, exported content is written to stdout. Use `--output` to write to a
file instead:

```bash
# Export to stdout
frankie --pr-url https://github.com/owner/repo/pull/123 --export markdown

# Export to file
frankie --pr-url https://github.com/owner/repo/pull/123 --export jsonl --output comments.jsonl
```

### Stable ordering

Comments are sorted in a stable, deterministic order:

1. By file path (alphabetically, missing paths sorted last)
2. By line number (ascending, missing line numbers sorted last)
3. By comment ID (ascending, for tie-breaking)

This ensures consistent output across runs for the same PR state.

### Markdown format example

The output includes a header, then each comment with location, reviewer, body,
and code context (if available):

```markdown
# Review Comments Export

PR: https://github.com/owner/repo/pull/123

---

## src/auth.rs:42

**Reviewer:** alice
**Created:** 2025-01-15T10:30:00Z

Consider using a constant here instead of a magic number.

```rust
@@ -40,3 +40,5 @@ fn validate_token(token: &str) -> bool {
-    token.len() > 0
+    token.len() > 8
 }
```

```---

### JSONL format example

```jsonl
{"id":456,"author":"alice","file_path":"src/auth.rs","line_number":42,"body":"Consider using a constant here.","diff_hunk":"@@ -40,3 +40,5 @@...","commit_sha":"abc123","created_at":"2025-01-15T10:30:00Z"}
{"id":457,"author":"bob","file_path":"src/auth.rs","line_number":50,"body":"Add error handling.","diff_hunk":"@@ -48,3 +48,5 @@...","commit_sha":"abc123","created_at":"2025-01-15T11:00:00Z"}
```

### Custom template format

For maximum flexibility, use a Jinja2-compatible template with the `template`
format:

```bash
frankie --pr-url https://github.com/owner/repo/pull/123 --export template \
        --template my-template.j2 --output comments.txt
```

#### Template syntax

Templates use Jinja2 syntax (via the `minijinja` engine):

- `{{ variable }}` — variable interpolation
- `{% for item in list %}...{% endfor %}` — loops
- `{{ list | length }}` — filters

#### Available variables

**Document-level** (available anywhere in the template):

Table: Document-level variables.

| Variable       | Description                 |
| -------------- | --------------------------- |
| `pr_url`       | Pull request URL            |
| `generated_at` | Export timestamp (ISO 8601) |
| `comments`     | List of comment objects     |

**Comment-level** (inside `{% for c in comments %}`):

Table: Comment-level variables.

| Variable      | Description                  |
| ------------- | ---------------------------- |
| `c.id`        | Comment ID                   |
| `c.file`      | File path                    |
| `c.line`      | Line number                  |
| `c.reviewer`  | Comment author               |
| `c.status`    | "reply" or "comment"         |
| `c.body`      | Comment text                 |
| `c.context`   | Diff hunk (code context)     |
| `c.commit`    | Commit SHA                   |
| `c.timestamp` | Creation timestamp           |
| `c.reply_to`  | Parent comment ID (if reply) |

#### Example template

```jinja2
# Comments for {{ pr_url }}
Generated: {{ generated_at }}

{% for c in comments %}
## {{ c.file }}:{{ c.line }} ({{ c.status }})

**By:** {{ c.reviewer }} at {{ c.timestamp }}

{{ c.body }}

---
{% endfor %}

Total: {{ comments | length }} comments
```

#### Template errors

- **Missing template file** — The `--template` flag is required when using
  `--export template`.
- **Invalid template syntax** — Verify the Jinja2 syntax (unclosed blocks,
  invalid expressions).

### Export errors

- **Missing PR URL** — The `--pr-url` flag is required when using `--export`.
- **Invalid format** — Use `markdown`, `jsonl`, or `template` as the export
  format value.
- **File write error** — Check that the output path is writable.

## Configuration

Frankie supports configuration through multiple sources with the following
precedence (lowest to highest):

1. **Defaults** — Built-in application defaults
2. **Configuration file** — `.frankie.toml` in current directory, home
   directory, or XDG config directory
3. **Environment variables** — `FRANKIE_*` variables or legacy `GITHUB_TOKEN`
4. **Command-line arguments** — command-line interface (CLI) flags take the
   highest precedence

Higher precedence sources override lower ones. For example, a CLI flag always
takes precedence over an environment variable or configuration file value.

### Configuration file

Create a `.frankie.toml` file in the current directory, home directory, or XDG
config directory (typically `~/.config/frankie/frankie.toml`):

```toml
# For single PR mode
pr_url = "https://github.com/owner/repo/pull/123"

# For repository listing mode
owner = "octocat"
repo = "hello-world"

# Authentication
token = "ghp_example"

# Local persistence (optional)
database_url = "frankie.sqlite"

# Pull request metadata cache time-to-live (TTL) (optional, seconds)
pr_metadata_cache_ttl_seconds = 86400

# Database migrations (set to true to run migrations and exit)
migrate_db = true
```

Frankie searches for configuration files in this order:

1. `.frankie.toml` in the current working directory
2. `.frankie.toml` in the home directory
3. `frankie.toml` in `$XDG_CONFIG_HOME/frankie/` (typically
   `~/.config/frankie/`)

### Environment variables

| Variable                                | Description                                         |
| --------------------------------------- | --------------------------------------------------- |
| `FRANKIE_PR_URL`                        | Pull request URL (for single PR mode)               |
| `FRANKIE_OWNER`                         | Repository owner (for listing mode)                 |
| `FRANKIE_REPO`                          | Repository name (for listing mode)                  |
| `FRANKIE_TOKEN`                         | GitHub personal access token                        |
| `FRANKIE_DATABASE_URL`                  | Local SQLite database path for persistence          |
| `FRANKIE_PR_METADATA_CACHE_TTL_SECONDS` | PR metadata cache TTL (seconds)                     |
| `FRANKIE_TEMPLATE`                      | Template file path for custom export format         |
| `GITHUB_TOKEN`                          | Legacy token variable (lower precedence than above) |

The `GITHUB_TOKEN` environment variable is supported for backward
compatibility. If both `FRANKIE_TOKEN` and `GITHUB_TOKEN` are set,
`FRANKIE_TOKEN` takes precedence.

### Command-line flags

| Flag                                        | Short | Description                                       |
| ------------------------------------------- | ----- | ------------------------------------------------- |
| `--pr-url <URL>`                            | `-u`  | GitHub pull request URL                           |
| `--owner <OWNER>`                           | `-o`  | Repository owner (user or organization)           |
| `--repo <REPO>`                             | `-r`  | Repository name                                   |
| `--token <TOKEN>`                           | `-t`  | Personal access token                             |
| `--database-url <PATH>`                     | —     | Local SQLite database path                        |
| `--migrate-db`                              | —     | Run database migrations and exit                  |
| `--pr-metadata-cache-ttl-seconds <SECONDS>` | —     | PR metadata cache TTL (seconds)                   |
| `--no-local-discovery`                      | `-n`  | Disable automatic local Git discovery             |
| `--tui`                                     | `-T`  | Launch interactive TUI for review comments        |
| `--export <FORMAT>`                         | `-e`  | Export comments (`markdown`, `jsonl`, `template`) |
| `--output <PATH>`                           | —     | Output file for export (default: stdout)          |
| `--template <PATH>`                         | —     | Template file for custom export format            |
| `--help`                                    | `-h`  | Show help information                             |

Run `frankie --help` to see all available options and their descriptions.

## Database migrations

Frankie ships Diesel migrations for its local SQLite schema. To apply any
pending migrations to a database file and record the resulting schema version
in telemetry, run:

```bash
frankie --migrate-db --database-url frankie.sqlite
```

This command does not contact GitHub. On success, Frankie records a telemetry
event as a JSON line on stderr.

For testing or ephemeral usage, SQLite supports an in-memory database using the
special `:memory:` URL:

```bash
frankie --migrate-db --database-url :memory:
```

## Local caching

When `--database-url` is set, Frankie caches pull request metadata in the
SQLite database and reuses it across sessions. Cached metadata is treated as
fresh until the configured TTL expires (default: 24 hours). Once expired,
Frankie revalidates the cache using conditional requests based on `ETag` and
`Last-Modified` headers when GitHub provides them.

To enable caching:

1. Run migrations once:

   ```bash
   frankie --migrate-db --database-url frankie.sqlite
   ```

2. Use the same database path when loading a pull request:

   ```bash
   frankie --pr-url https://github.com/owner/repo/pull/123 --database-url frankie.sqlite
   ```

To change the TTL, set `--pr-metadata-cache-ttl-seconds` (or
`FRANKIE_PR_METADATA_CACHE_TTL_SECONDS`).

## Error handling

Authentication or network failures set the process exit code to a non-zero
value and emit a clear error message describing the failing step (e.g. "GitHub
rejected the token: Bad credentials").

Common error scenarios:

- **Missing required arguments** — Either `--pr-url` or both `--owner` and
  `--repo` must be provided.
- **Authentication failure** — Token is missing, empty, or rejected by GitHub.
- **Rate limit exceeded** — API rate limit reached; wait for reset time.
- **Network errors** — Cannot reach the GitHub API endpoint.
