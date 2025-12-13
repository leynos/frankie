# User guide

This guide explains how to use Frankie for GitHub pull request workflows,
including loading individual pull requests and listing PRs for a repository.

## Prerequisites

- A GitHub personal access token (classic or fine-grained) with permission to
  read pull requests and comments in the target repository.
- Network access to the GitHub host referenced by the pull request or
  repository URL.

## Operation modes

Frankie supports two operation modes:

1. **Single pull request mode** — Load a specific PR by URL using `--pr-url`
2. **Repository listing mode** — List PRs for a repository using `--owner` and
   `--repo`

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

- `--owner` (or `-o`) specifies the repository owner (user or organisation).
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
```

Frankie searches for configuration files in this order:

1. `.frankie.toml` in the current working directory
2. `.frankie.toml` in the home directory
3. `frankie.toml` in `$XDG_CONFIG_HOME/frankie/` (typically
   `~/.config/frankie/`)

### Environment variables

| Variable         | Description                                         |
| ---------------- | --------------------------------------------------- |
| `FRANKIE_PR_URL` | Pull request URL (for single PR mode)               |
| `FRANKIE_OWNER`  | Repository owner (for listing mode)                 |
| `FRANKIE_REPO`   | Repository name (for listing mode)                  |
| `FRANKIE_TOKEN`  | GitHub personal access token                        |
| `GITHUB_TOKEN`   | Legacy token variable (lower precedence than above) |

The `GITHUB_TOKEN` environment variable is supported for backward
compatibility. If both `FRANKIE_TOKEN` and `GITHUB_TOKEN` are set,
`FRANKIE_TOKEN` takes precedence.

### Command-line flags

| Flag              | Short | Description                             |
| ----------------- | ----- | --------------------------------------- |
| `--pr-url <URL>`  | `-u`  | GitHub pull request URL                 |
| `--owner <OWNER>` | `-o`  | Repository owner (user or organisation) |
| `--repo <REPO>`   | `-r`  | Repository name                         |
| `--token <TOKEN>` | `-t`  | Personal access token                   |
| `--help`          | `-h`  | Show help information                   |

Run `frankie --help` to see all available options and their descriptions.

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
