# User guide

This guide explains how to use Frankie for GitHub pull request workflows,
including loading individual pull requests and listing PRs for a repository.

## Prerequisites

- A GitHub personal access token (classic or fine-grained) with permission to
  read pull requests and comments in the target repository.
- Network access to the GitHub host referenced by the pull request or
  repository URL.

## Operation modes

Frankie supports four operation modes:

1. **Interactive mode** — Auto-detect repository from local Git directory
2. **Single pull request mode** — Load a specific PR by URL using `--pr-url`
3. **Repository listing mode** — List PRs for a repository using `--owner` and
   `--repo`
4. **Review TUI mode** — Interactive terminal interface for navigating and
   filtering review comments using `--tui` with `--pr-url`

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

| Key         | Action                        |
| ----------- | ----------------------------- |
| `j`, `↓`    | Move cursor down              |
| `k`, `↑`    | Move cursor up                |
| `PgDn`      | Page down                     |
| `PgUp`      | Page up                       |
| `Home`, `g` | Go to first item              |
| `End`, `G`  | Go to last item               |
| `f`         | Cycle filter (All/Unresolved) |
| `Esc`       | Clear filter or exit context  |
| `c`         | Open full-screen diff context |
| `[`         | Previous diff hunk            |
| `]`         | Next diff hunk                |
| `r`         | Refresh from GitHub           |
| `?`         | Toggle help overlay           |
| `q`         | Quit                          |

### Background sync

The TUI automatically refreshes review comments from GitHub every 30 seconds.
During a background sync:

- New comments are added to the list
- Updated comments are refreshed
- Deleted comments are removed
- The current selection is preserved (unless the selected comment was deleted)

A `[Loading…]` indicator appears in the header during sync. Manual refresh with
`r` uses the same incremental sync logic.

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
| `GITHUB_TOKEN`                          | Legacy token variable (lower precedence than above) |

The `GITHUB_TOKEN` environment variable is supported for backward
compatibility. If both `FRANKIE_TOKEN` and `GITHUB_TOKEN` are set,
`FRANKIE_TOKEN` takes precedence.

### Command-line flags

| Flag                                        | Short | Description                                |
| ------------------------------------------- | ----- | ------------------------------------------ |
| `--pr-url <URL>`                            | `-u`  | GitHub pull request URL                    |
| `--owner <OWNER>`                           | `-o`  | Repository owner (user or organization)    |
| `--repo <REPO>`                             | `-r`  | Repository name                            |
| `--token <TOKEN>`                           | `-t`  | Personal access token                      |
| `--database-url <PATH>`                     | —     | Local SQLite database path                 |
| `--migrate-db`                              | —     | Run database migrations and exit           |
| `--pr-metadata-cache-ttl-seconds <SECONDS>` | —     | PR metadata cache TTL (seconds)            |
| `--no-local-discovery`                      | `-n`  | Disable automatic local Git discovery      |
| `--tui`                                     | `-T`  | Launch interactive TUI for review comments |
| `--help`                                    | `-h`  | Show help information                      |

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
