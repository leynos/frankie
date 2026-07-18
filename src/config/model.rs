//! Definition of [`FrankieConfig`], its defaults, and default constants.

use ortho_config::OrthoConfig;
use serde::{Deserialize, Serialize};

/// Application configuration supporting CLI, environment, and file sources.
///
/// # Environment Variables
///
/// - `FRANKIE_PR_URL` or `--pr-url`: Pull request URL
/// - `FRANKIE_TOKEN`, `GITHUB_TOKEN`, or `--token`: Authentication token
/// - `FRANKIE_OWNER` or `--owner`: Repository owner
/// - `FRANKIE_REPO` or `--repo`: Repository name
/// - `FRANKIE_DATABASE_URL` or `--database-url`: Local `SQLite` database path
/// - `FRANKIE_TEMPLATE` or `--template`: Template file path for custom export
/// - `FRANKIE_REPLY_MAX_LENGTH` or `--reply-max-length`: Max reply length
/// - `FRANKIE_REPLY_TEMPLATES` or `--reply-templates`: Reply templates
/// - `FRANKIE_AI_REWRITE_MODE` or `--ai-rewrite-mode`: Rewrite mode
/// - `FRANKIE_AI_REWRITE_TEXT` or `--ai-rewrite-text`: Source draft text
/// - `FRANKIE_AI_BASE_URL` or `--ai-base-url`: AI API base URL
/// - `FRANKIE_AI_MODEL` or `--ai-model`: AI model identifier
/// - `FRANKIE_AI_API_KEY`, `OPENAI_API_KEY`, or `--ai-api-key`: AI API key
/// - `FRANKIE_AI_TIMEOUT_SECONDS` or `--ai-timeout-seconds`: Request timeout
/// - `--verify-resolutions`: Verify resolutions
///
/// # Example
///
/// ```no_run
/// use frankie::FrankieConfig;
/// use ortho_config::OrthoConfig;
///
/// let config = FrankieConfig::load().expect("failed to load configuration");
/// let pr_url = config.require_pr_url().expect("PR URL required");
/// let token = config.resolve_token().expect("token required");
/// ```
#[derive(Debug, Clone, Deserialize, Serialize, OrthoConfig)]
#[serde(default)]
#[ortho_config(
    prefix = "FRANKIE",
    discovery(
        dotfile_name = ".frankie.toml",
        config_file_name = "frankie.toml",
        app_name = "frankie"
    )
)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "Configuration models independent CLI/config switches; refactoring into nested enums would either break the existing CLI surface or require ortho_config macro changes."
)]
pub struct FrankieConfig {
    /// GitHub pull request URL to load.
    ///
    /// Can be provided via:
    /// - CLI: `--pr-url <URL>` or `-u <URL>`
    /// - Environment: `FRANKIE_PR_URL`
    /// - Config file: `pr_url = "..."`
    #[ortho_config(cli_short = 'u')]
    pub pr_url: Option<String>,

    /// Personal access token for GitHub API authentication.
    ///
    /// Can be provided via:
    /// - CLI: `--token <TOKEN>` or `-t <TOKEN>`
    /// - Environment: `FRANKIE_TOKEN` or `GITHUB_TOKEN` (legacy)
    /// - Config file: `token = "..."`
    #[ortho_config(cli_short = 't')]
    pub token: Option<String>,

    /// Repository owner (e.g., "octocat").
    ///
    /// Can be provided via:
    /// - CLI: `--owner <OWNER>` or `-o <OWNER>`
    /// - Environment: `FRANKIE_OWNER`
    /// - Config file: `owner = "..."`
    #[ortho_config(cli_short = 'o')]
    pub owner: Option<String>,

    /// Repository name (e.g., "hello-world").
    ///
    /// Can be provided via:
    /// - CLI: `--repo <REPO>` or `-r <REPO>`
    /// - Environment: `FRANKIE_REPO`
    /// - Config file: `repo = "..."`
    #[ortho_config(cli_short = 'r')]
    pub repo: Option<String>,

    /// Local `SQLite` database URL/path used for persistence.
    ///
    /// Diesel uses a filesystem path for `SQLite` connections. The same value is
    /// also used by the Diesel CLI via `DATABASE_URL` when running migrations.
    ///
    /// Can be provided via:
    /// - CLI: `--database-url <PATH>`
    /// - Environment: `FRANKIE_DATABASE_URL`
    /// - Config file: `database_url = "..."`
    #[ortho_config()]
    pub database_url: Option<String>,

    /// Runs database migrations and exits.
    ///
    /// When set, Frankie initializes the database at `database_url`, applies
    /// any pending Diesel migrations, records the schema version in telemetry,
    /// and exits without performing GitHub operations.
    ///
    /// Can be provided via:
    /// - CLI: `--migrate-db`
    /// - Environment: `FRANKIE_MIGRATE_DB`
    /// - Config file: `migrate_db = true`
    #[ortho_config()]
    pub migrate_db: bool,

    /// TTL for cached pull request metadata, in seconds.
    ///
    /// When `database_url` is set, Frankie can cache pull request metadata in
    /// the local `SQLite` database and reuse it across sessions. Entries are
    /// treated as fresh until this TTL expires, after which Frankie performs a
    /// conditional request using stored `ETag` / `Last-Modified` validators when
    /// available.
    ///
    /// Defaults to 24 hours.
    #[ortho_config()]
    pub pr_metadata_cache_ttl_seconds: u64,

    /// Disables automatic local repository discovery.
    ///
    /// When set to true, Frankie will not attempt to detect owner/repo from
    /// the current Git repository even when running in Interactive mode.
    ///
    /// Can be provided via:
    /// - CLI: `--no-local-discovery` / `-n`
    /// - Config file: `no_local_discovery = true`
    ///
    /// Note: Environment variable `FRANKIE_NO_LOCAL_DISCOVERY` is not supported
    /// because `ortho_config` does not load boolean values from the environment.
    #[ortho_config(cli_short = 'n')]
    pub no_local_discovery: bool,

    /// Enables interactive TUI mode for reviewing PR comments.
    ///
    /// When set, Frankie launches a terminal user interface for navigating
    /// and filtering review comments on a pull request.
    ///
    /// Can be provided via:
    /// - CLI: `--tui` / `-T`
    /// - Config file: `tui = true`
    #[ortho_config(cli_short = 'T')]
    pub tui: bool,

    /// Export format for review comments.
    ///
    /// When set, Frankie exports review comments in the specified format
    /// instead of displaying them interactively. Valid values are `markdown`,
    /// `jsonl`, and `template` (requires `--template` to specify a template file).
    ///
    /// Can be provided via:
    /// - CLI: `--export <FORMAT>` or `-e <FORMAT>`
    /// - Environment: `FRANKIE_EXPORT`
    /// - Config file: `export = "markdown"`
    #[ortho_config(cli_short = 'e')]
    pub export: Option<String>,

    /// Runs automated resolution verification and exits.
    ///
    /// When set, Frankie loads review comments for the pull request and
    /// verifies whether the referenced code has changed between the comment's
    /// commit and the local repository `HEAD`.
    ///
    /// Verification requires both a local git repository (`--repo-path` or
    /// local discovery) and a migrated `SQLite` database (`--database-url`) so
    /// results can be persisted locally.
    ///
    /// Can be provided via:
    /// - CLI: `--verify-resolutions`
    /// - Config file: `verify_resolutions = true`
    ///
    /// Note: Environment variable `FRANKIE_VERIFY_RESOLUTIONS` is not
    /// supported because `ortho_config` does not load boolean values from the
    /// environment.
    #[ortho_config()]
    pub verify_resolutions: bool,

    /// Runs PR-level discussion summary generation and exits.
    ///
    /// When set, Frankie loads review comments for the selected pull request,
    /// groups them into discussion threads, asks the configured AI provider to
    /// summarize those threads, and prints file- and severity-grouped output.
    ///
    /// Can be provided via:
    /// - CLI: `--summarize-discussions`
    /// - Config file: `summarize_discussions = true`
    ///
    /// Note: Environment variable `FRANKIE_SUMMARIZE_DISCUSSIONS` is not
    /// supported because `ortho_config` does not load boolean values from the
    /// environment.
    #[ortho_config()]
    pub summarize_discussions: bool,

    /// Output file path for exported comments.
    ///
    /// When set, Frankie writes exported comments to the specified file
    /// instead of stdout. Requires `--export` to be set.
    ///
    /// Can be provided via:
    /// - CLI: `--output <PATH>`
    /// - Environment: `FRANKIE_OUTPUT`
    /// - Config file: `output = "comments.md"`
    #[ortho_config()]
    pub output: Option<String>,

    /// Template file path for custom export format.
    ///
    /// When `--export template` is used, this specifies the Jinja2-compatible
    /// template file. The template uses `minijinja` syntax with placeholders
    /// for comment fields (`{{ file }}`, `{{ line }}`, `{{ reviewer }}`, etc.).
    ///
    /// Can be provided via:
    /// - CLI: `--template <PATH>`
    /// - Environment: `FRANKIE_TEMPLATE`
    /// - Config file: `template = "my-template.j2"`
    #[ortho_config()]
    pub template: Option<String>,

    /// Local repository path for time-travel features (`--repo-path`,
    /// `FRANKIE_REPO_PATH`). Overrides auto-discovery from the current
    /// working directory.
    #[ortho_config()]
    pub repo_path: Option<String>,

    /// Maximum character count allowed for TUI reply drafts.
    ///
    /// Reply drafting enforces this limit while typing and during template
    /// insertion. Character counting is based on Unicode scalar values.
    ///
    /// Can be provided via:
    /// - CLI: `--reply-max-length <COUNT>`
    /// - Environment: `FRANKIE_REPLY_MAX_LENGTH`
    /// - Config file: `reply_max_length = 500`
    #[ortho_config()]
    pub reply_max_length: usize,

    /// Ordered template list used for keyboard reply insertion in the TUI.
    ///
    /// Templates are rendered with `MiniJinja` and can reference review-comment
    /// variables: `comment_id`, `reviewer`, `file`, `line`, and `body`.
    ///
    /// Can be provided via:
    /// - CLI: `--reply-templates '<json-array>'`
    /// - Environment: `FRANKIE_REPLY_TEMPLATES`
    /// - Config file:
    ///   `reply_templates = ["Thanks {{ reviewer }}", "..."]`
    #[ortho_config()]
    pub reply_templates: Vec<String>,

    /// Rewrite mode for non-interactive AI draft rewriting.
    ///
    /// Valid values are `expand` and `reword`.
    #[ortho_config()]
    pub ai_rewrite_mode: Option<String>,

    /// Source draft text for non-interactive AI draft rewriting.
    #[ortho_config()]
    pub ai_rewrite_text: Option<String>,

    /// Base URL for the OpenAI-compatible rewrite API.
    ///
    /// Defaults to `https://api.openai.com/v1`.
    #[ortho_config()]
    pub ai_base_url: String,

    /// Model identifier used for rewrite requests.
    ///
    /// Defaults to `gpt-4o-mini`.
    #[ortho_config()]
    pub ai_model: String,

    /// API key for OpenAI-compatible rewrite requests.
    #[ortho_config()]
    pub ai_api_key: Option<String>,

    /// Timeout for AI rewrite requests, in seconds.
    #[ortho_config()]
    pub ai_timeout_seconds: u64,

    /// Maximum number of commits to load in time-travel history.
    ///
    /// Controls how many parent commits are retrieved when entering
    /// time-travel mode. Larger values provide more navigation depth
    /// at the cost of increased load time for repositories with long
    /// histories.
    ///
    /// Must be at least 1; a value of 0 is clamped to 1 by calling
    /// `FrankieConfig::normalize()`, which is invoked automatically
    /// by all public load methods.
    ///
    /// Can be provided via:
    /// - CLI: `--commit-history-limit <COUNT>`
    /// - Environment: `FRANKIE_COMMIT_HISTORY_LIMIT`
    /// - Config file: `commit_history_limit = 50`
    #[ortho_config()]
    pub commit_history_limit: usize,

    /// Positional PR identifier (bare number or full URL) extracted from
    /// command-line arguments before ortho-config processes the remaining
    /// flags. When set, the TUI is launched without requiring `-T`.
    #[serde(skip)]
    pub pr_identifier: Option<String>,
}

const DEFAULT_PR_METADATA_CACHE_TTL_SECONDS: u64 = 86_400;
pub(crate) const DEFAULT_REPLY_MAX_LENGTH: usize = 500;
pub(crate) const DEFAULT_AI_BASE_URL: &str = "https://api.openai.com/v1";
pub(crate) const DEFAULT_AI_MODEL: &str = "gpt-4o-mini";
pub(crate) const DEFAULT_AI_TIMEOUT_SECONDS: u64 = 20;

/// Default maximum number of commits to load in time-travel history.
pub const DEFAULT_COMMIT_HISTORY_LIMIT: usize = 50;

impl Default for FrankieConfig {
    fn default() -> Self {
        Self {
            pr_url: None,
            token: None,
            owner: None,
            repo: None,
            database_url: None,
            migrate_db: false,
            pr_metadata_cache_ttl_seconds: DEFAULT_PR_METADATA_CACHE_TTL_SECONDS,
            no_local_discovery: false,
            tui: false,
            export: None,
            verify_resolutions: false,
            summarize_discussions: false,
            output: None,
            template: None,
            repo_path: None,
            reply_max_length: DEFAULT_REPLY_MAX_LENGTH,
            reply_templates: default_reply_templates(),
            ai_rewrite_mode: None,
            ai_rewrite_text: None,
            ai_base_url: DEFAULT_AI_BASE_URL.to_owned(),
            ai_model: DEFAULT_AI_MODEL.to_owned(),
            ai_api_key: None,
            ai_timeout_seconds: DEFAULT_AI_TIMEOUT_SECONDS,
            commit_history_limit: DEFAULT_COMMIT_HISTORY_LIMIT,
            pr_identifier: None,
        }
    }
}

pub(crate) fn default_reply_templates() -> Vec<String> {
    vec![
        "Thanks for the review on {{ file }}:{{ line }}. I will update this.".to_owned(),
        "Good catch, {{ reviewer }}. I will address this in the next commit.".to_owned(),
        "I have addressed this feedback and pushed an update.".to_owned(),
    ]
}
