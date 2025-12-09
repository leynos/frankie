# User guide

This guide explains how to load a GitHub pull request in Frankie and what to
expect from the current intake workflow.

- Frankie reads a pull request URL and a personal access token, then fetches
  the pull request metadata and all issue comments.
- Authentication is performed through Octocrab; HTTP 401 or 403 responses are
  surfaced as a clear authentication failure that includes the GitHub error
  message.
- The CLI summarizes the pull request number, author, title, HTML URL and the
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
- `--token` (or `-t`) is optional when the `FRANKIE_TOKEN` or `GITHUB_TOKEN`
  environment variable is set. An empty token fails fast with a user-readable
  error.
- Hosts other than `github.com` are treated as GitHub Enterprise servers by
  default; the API base is derived as `https://<host>/api/v3`.

Run `frankie --help` to see all available options and their descriptions.

## Configuration

Frankie supports configuration through multiple sources with the following
precedence (lowest to highest):

1. **Defaults** - Built-in application defaults
2. **Configuration file** - `.frankie.toml` in current directory, home
   directory, or XDG config directory
3. **Environment variables** - `FRANKIE_PR_URL`, `FRANKIE_TOKEN`, or legacy
   `GITHUB_TOKEN`
4. **Command-line arguments** - `--pr-url`/`-u` and `--token`/`-t`

Higher precedence sources override lower ones. For example, a CLI flag always
takes precedence over an environment variable or configuration file value.

### Configuration file

Create a `.frankie.toml` file in your current directory, home directory, or XDG
config directory (typically `~/.config/frankie/frankie.toml`):

```toml
pr_url = "https://github.com/owner/repo/pull/123"
token = "ghp_example"
```

Frankie searches for configuration files in this order:

1. `.frankie.toml` in the current working directory
2. `.frankie.toml` in your home directory
3. `frankie.toml` in `$XDG_CONFIG_HOME/frankie/` (typically
   `~/.config/frankie/`)

### Environment variables

| Variable         | Description                                         |
| ---------------- | --------------------------------------------------- |
| `FRANKIE_PR_URL` | Pull request URL                                    |
| `FRANKIE_TOKEN`  | GitHub personal access token                        |
| `GITHUB_TOKEN`   | Legacy token variable (lower precedence than above) |

The `GITHUB_TOKEN` environment variable is supported for backward
compatibility. If both `FRANKIE_TOKEN` and `GITHUB_TOKEN` are set,
`FRANKIE_TOKEN` takes precedence.

### Command-line flags

| Flag              | Short | Description                        |
| ----------------- | ----- | ---------------------------------- |
| `--pr-url <URL>`  | `-u`  | GitHub pull request URL (required) |
| `--token <TOKEN>` | `-t`  | Personal access token              |
| `--help`          | `-h`  | Show help information              |

## Expected output

A successful call prints a short summary:

```text
Loaded PR #123 by octocat: Add search
URL: https://github.com/owner/repo/pull/123
Comments: 2
```

Authentication or network failures set the process exit code to a non-zero
value and emit a clear error message describing the failing step (e.g. "GitHub
rejected the token: Bad credentials").
