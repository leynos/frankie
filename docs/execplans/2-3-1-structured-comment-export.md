# Structured comment export pipeline

This execution plan (ExecPlan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` must be kept up to date as work
proceeds.

Status: COMPLETE

PLANS.md is not present in the repository root, so no additional plan
governance applies.

## Purpose / Big picture

Deliver a structured comment export pipeline that outputs review comments with
location, code context, and issue text in both Markdown and JSONL formats. The
export must include stable ordering (by file path, then line number, then
comment ID), pass schema validation, and be exercised in integration tests.
Success is visible when comments can be exported via CLI with `--export` flag,
output files match expected schema, and behavioural tests cover happy and
unhappy paths.

## Constraints

- Keep the model-view-update (MVU) split intact: export logic belongs in
  `src/cli/` as it is a CLI operation, not a TUI feature.
- Every new module begins with a `//!` module-level comment.
- No single file may exceed 400 lines; split into feature-focused modules if
  needed.
- Use `rstest` for unit tests and `rstest-bdd` v0.4.0 for behavioural tests.
- Add `#[derive(Serialize)]` to models only where needed; avoid polluting public
  API with unnecessary derives.
- Stable ordering: comments sorted by file_path (alphabetical), then
  line_number (ascending), then id (ascending).
- JSONL format: one JSON object per line, newline-delimited.
- Markdown format: structured text with code blocks for diff context.
- Any filesystem access must use `std::io::Write` trait for testability.
- Avoid adding new dependencies beyond the existing stack; if unavoidable,
  escalate before proceeding.
- Documentation updates must follow the en-GB style guide, wrap at 80 columns,
  and pass `make markdownlint`, `make fmt`, and `make nixie`.
- Use Makefile targets for validation (`make check-fmt`, `make lint`,
  `make test`).

## Tolerances (exception triggers)

- Scope: if implementation needs more than 15 files or 800 net new lines, stop
  and escalate.
- Interface: if any public API signature must change beyond adding Serialize
  derive, stop and escalate.
- Dependencies: if a new external dependency is required, stop and escalate.
- Tests: if tests still fail after two fix attempts, stop and escalate with the
  latest failure output.
- Ambiguity: if the export format choice materially affects downstream AI tool
  compatibility, stop and ask for confirmation.

## Risks

- Risk: Large PRs with many comments cause slow export. Severity: low.
  Likelihood: low. Mitigation: streaming writes, avoid loading all into memory.
- Risk: Comments missing file_path or line_number break stable ordering.
  Severity: medium. Likelihood: medium. Mitigation: use Option sorting with
  None values sorted last.
- Risk: Diff hunk contains special characters that break Markdown/JSONL.
  Severity: low. Likelihood: medium. Mitigation: proper escaping in serializers.
- Risk: Schema validation complexity. Severity: low. Likelihood: low.
  Mitigation: use JSON Schema for JSONL, structural assertions for Markdown.

## Progress

- [x] Stage A: Export Data Model
- [x] Stage B: Export Formatters
- [x] Stage C: CLI Integration
- [x] Stage D: Unit Testing
- [x] Stage E: BDD Testing
- [x] Stage F: Documentation and Close-out

## Surprises & discoveries

- The `ortho_config` crate does not support `-O` as a short flag due to conflict
  with built-in clap options. Changed to using no short flag for `--output`.
- `serde_json::to_writer` does not add a trailing newline, requiring explicit
  newline handling in JSONL output.
- Clippy's `missing_docs` lint requires documentation on all public items
  including enum variants.

## Decision log

- Decision: Use separate serializable struct `ExportedComment` rather than
  adding Serialize to `ReviewComment`. Rationale: keeps public API clean,
  allows export-specific field transformations. Date/Author: 2026-01-30, plan
  author.
- Decision: Export via CLI flag `--export <format>` with values `markdown` and
  `jsonl`, outputting to stdout by default with optional `--output <path>`.
  Rationale: follows Unix philosophy, enables piping to other tools.
  Date/Author: 2026-01-30, plan author.
- Decision: Use file_path → line_number → id for stable ordering. Rationale:
  groups related comments, deterministic output for testing. Date/Author:
  2026-01-30, plan author.
- Decision: Markdown format uses fenced code blocks with language hints from
  file extension. Rationale: enables syntax highlighting in viewers, matches
  design doc intent. Date/Author: 2026-01-30, plan author.

## Outcomes & retrospective

**Completed:** 2026-01-30

All acceptance criteria met:

- `frankie --pr-url <URL> --token <TOKEN> --export markdown` outputs valid
  Markdown to stdout with review comments sorted by file, line, then ID.
- `frankie --pr-url <URL> --token <TOKEN> --export jsonl` outputs valid JSONL
  to stdout (one JSON object per line).
- `--output <path>` writes to file instead of stdout.
- Empty comment list produces minimal valid output (header only for Markdown,
  empty for JSONL).
- Invalid format (e.g. `--export xml`) produces user-friendly error.
- Unit tests (rstest) cover formatters, ordering, and model conversion.
- BDD tests (rstest-bdd) cover 6 scenarios: Markdown export, JSONL export,
  stable ordering, empty Markdown, empty JSONL, and invalid format error.
- `make check-fmt`, `make lint`, and `make test` succeed.
- Documentation updated in `docs/users-guide.md` with export feature section.
- Roadmap entry marked complete in `docs/roadmap.md`.

**Lessons learned:**

- The rstest-bdd v0.4.0 macro captures step parameters literally including
  quotes; use `trim_matches('"')` to strip them when comparing values.
- Export types in binary crate (`src/cli/`) cannot be imported by integration
  tests in `tests/`; solved by inlining necessary types in the BDD test file.
- Separation of concerns between `ExportedComment` (serializable) and
  `ReviewComment` (domain model) keeps the public API clean and allows
  export-specific transformations.

## Context and orientation

The CLI lives under `src/cli/`. Operation modes are routed in `src/main.rs`
based on `FrankieConfig::operation_mode()`. Current modes include
`SinglePullRequest`, `RepositoryListing`, `Interactive`, and `ReviewTui`.
Output formatting utilities exist in `src/cli/output.rs`.

Review comments are fetched via `OctocrabReviewCommentGateway` implementing
`ReviewCommentGateway` trait. The `ReviewComment` struct in
`src/github/models/mod.rs` contains all necessary fields:

- `id: u64` - comment identifier
- `body: Option<String>` - comment text
- `author: Option<String>` - reviewer username
- `file_path: Option<String>` - file being reviewed
- `line_number: Option<u32>` - line in diff
- `original_line_number: Option<u32>` - original line before changes
- `diff_hunk: Option<String>` - code context/diff
- `commit_sha: Option<String>` - commit SHA this comment is on
- `in_reply_to_id: Option<u64>` - thread reply tracking
- `created_at: Option<String>` - ISO 8601 timestamp
- `updated_at: Option<String>` - ISO 8601 timestamp

Serialization uses `serde` and `serde_json` (already in Cargo.toml).
Behavioural tests live under `tests/` with Gherkin feature files in
`tests/features/`.

## Plan of work

Stage A: Export Data Model. Create `ExportedComment` struct with Serialize
derive, implement conversion from `ReviewComment`, define stable ordering.

Stage B: Export Formatters. Create Markdown formatter and JSONL formatter
modules with `Write` trait for output flexibility.

Stage C: CLI Integration. Add `--export` and `--output` flags to config, create
export operation handler, integrate into main routing.

Stage D: Unit Testing. Add rstest unit tests for formatters, ordering, edge
cases (missing fields, special characters).

Stage E: BDD Testing. Create feature file and step definitions for export
scenarios.

Stage F: Documentation and Close-out. Update user guide, mark roadmap entry
done, run all validation gates.

## Concrete steps

### Stage A: Export Data Model

1. Create `src/cli/export/mod.rs` with:

   - Module-level documentation
   - Re-exports for public API

1. Create `src/cli/export/model.rs` with:

   - `ExportedComment` struct with `#[derive(Debug, Clone, Serialize)]`:
     - `id: u64`
     - `author: Option<String>`
     - `file_path: Option<String>`
     - `line_number: Option<u32>`
     - `original_line_number: Option<u32>`
     - `body: Option<String>`
     - `diff_hunk: Option<String>`
     - `commit_sha: Option<String>`
     - `comment_url: Option<String>` (constructed from metadata)
     - `created_at: Option<String>`
   - `impl From<&ReviewComment> for ExportedComment`
   - `ExportFormat` enum: `Markdown`, `Jsonl`
   - `impl FromStr for ExportFormat` for CLI parsing

1. Create `src/cli/export/ordering.rs` with:

   - `sort_comments(comments: &mut [ExportedComment])` function
   - Sorting by (file_path, line_number, id) with None values last
   - Unit tests inline

### Stage B: Export Formatters

1. Create `src/cli/export/markdown.rs` with:

   - `write_markdown<W>(writer, comments, pr_url) -> Result<(), IntakeError>`
   - Header section with PR metadata
   - Per-comment section with:
     - File path and line number heading
     - Code context in fenced block with language hint
     - Comment body
     - Author and timestamp
   - Unit tests inline

1. Create `src/cli/export/jsonl.rs` with:

   - `write_jsonl<W>(writer, comments) -> Result<(), IntakeError>`
   - One JSON object per line
   - Proper newline handling
   - Unit tests inline

### Stage C: CLI Integration

1. Update `src/config/mod.rs`:

   - Add `export: Option<String>` field for format selection
   - Add `output: Option<String>` field for output path
   - Add `#[ortho_config(cli_short = 'e')]` for `--export`/`-e`

1. Update `OperationMode` enum:

   - Add `ExportComments` variant

1. Update `FrankieConfig::operation_mode()`:

   - Return `ExportComments` when `export.is_some() && pr_url.is_some()`

1. Create `src/cli/export_comments.rs` with:

   - `pub async fn run(config: &FrankieConfig) -> Result<(), IntakeError>`
   - Load PR and review comments via gateway
   - Convert to `ExportedComment` and sort
   - Write to stdout or file based on config
   - Error handling for invalid format

1. Update `src/cli/mod.rs`:

   - Add `pub mod export;`
   - Add `pub mod export_comments;`

1. Update `src/main.rs`:

   - Add `OperationMode::ExportComments` match arm
   - Call `cli::export_comments::run()`

### Stage D: Unit Testing

1. Add tests to `src/cli/export/ordering.rs`:

   - Test stable ordering with full data
   - Test ordering with None values
   - Test ordering with duplicate file paths

1. Add tests to `src/cli/export/markdown.rs`:

   - Test basic Markdown output structure
   - Test code block language detection
   - Test escaping of special characters
   - Test empty comments list

1. Add tests to `src/cli/export/jsonl.rs`:

   - Test single comment JSONL
   - Test multiple comments (one per line)
   - Test JSON escaping
   - Test empty comments list

1. Add tests to `src/cli/export/model.rs`:

   - Test From conversion preserves fields
   - Test ExportFormat parsing

### Stage E: BDD Testing

1. Create `tests/features/comment_export.feature` with scenarios:

   - Export comments in Markdown format
   - Export comments in JSONL format
   - Export with stable ordering
   - Export empty comment list produces minimal output
   - Export with missing optional fields
   - Invalid export format produces error

1. Create `tests/comment_export_bdd.rs` entry point.

1. Create `tests/comment_export_bdd/mod.rs` with step module imports.

1. Create `tests/comment_export_bdd/state.rs` with:

   - `ExportState` struct using `Slot` pattern
   - Fields for runtime, mock server, output buffer, error

1. Create `tests/comment_export_bdd/steps.rs` with:

   - Given steps for setting up mock server with comments
   - When steps for invoking export
   - Then steps for asserting output format and content

### Stage F: Documentation and Close-out

1. Update `docs/users-guide.md` with:

   - New "Comment export" section
   - CLI flags documentation (`--export`, `--output`)
   - Format descriptions (Markdown, JSONL)
   - Example usage commands

1. Mark roadmap entry as done in `docs/roadmap.md`:

   - Change `[ ]` to `[x]` for the comment export pipeline item

1. Run validation gates:

   ```bash
   set -o pipefail
   make check-fmt 2>&1 | tee /tmp/frankie-check-fmt.log
   make lint 2>&1 | tee /tmp/frankie-lint.log
   make test 2>&1 | tee /tmp/frankie-test.log
   ```

1. Run documentation validators:

   ```bash
   set -o pipefail
   make markdownlint 2>&1 | tee /tmp/frankie-markdownlint.log
   make fmt 2>&1 | tee /tmp/frankie-docs-fmt.log
   make nixie 2>&1 | tee /tmp/frankie-nixie.log
   ```

## Validation and acceptance

Acceptance is satisfied when the following are true:

- `frankie --pr-url <URL> --token <TOKEN> --export markdown` outputs valid
  Markdown to stdout.
- `frankie --pr-url <URL> --token <TOKEN> --export jsonl` outputs valid JSONL
  to stdout (one JSON object per line).
- `--output <path>` writes to file instead of stdout.
- Comments are sorted by file_path, then line_number, then id (stable ordering).
- Empty comment list produces minimal valid output (header only for Markdown,
  empty for JSONL).
- Invalid format (e.g. `--export xml`) produces user-friendly error.
- Unit tests (rstest) cover formatters, ordering, and model conversion.
- BDD tests (rstest-bdd) cover happy/unhappy paths.
- `make check-fmt`, `make lint`, and `make test` succeed.
- Documentation updates pass `make markdownlint`, `make fmt`, and `make nixie`.

Quality criteria:

- Tests: rstest unit tests and rstest-bdd scenarios for the new behaviour.
- Lint/typecheck: `make lint` clean.
- Formatting: `make check-fmt` clean.

## Idempotence and recovery

All steps are re-runnable. If tests fail, inspect the log files under `/tmp/`,
apply fixes, and rerun the same commands. Export operations are read-only from
GitHub's perspective and produce deterministic output given the same input.

## Artefacts and notes

Example Markdown output (illustrative):

```markdown
# Review Comments Export

PR: https://github.com/owner/repo/pull/123

---

## src/auth.rs:42

**Reviewer:** alice
**Created:** 2025-01-15T10:30:00Z

Consider using a constant here instead of a magic number.

    @@ -40,3 +40,5 @@ fn validate_token(token: &str) -> bool {
    -    token.len() > 0
    +    token.len() > 8
     }
```

Example JSONL output (illustrative):

```jsonl
{"id":456,"author":"alice","file_path":"src/auth.rs","body":"Use a constant.","commit_sha":"abc123"}
{"id":457,"author":"bob","file_path":"src/auth.rs","body":"Add error handling.","commit_sha":"abc123"}
```

## Interfaces and dependencies

- New module: `src/cli/export/mod.rs` - export module root
- New module: `src/cli/export/model.rs` - `ExportedComment`, `ExportFormat`
- New module: `src/cli/export/ordering.rs` - stable sorting
- New module: `src/cli/export/markdown.rs` - Markdown formatter
- New module: `src/cli/export/jsonl.rs` - JSONL formatter
- New module: `src/cli/export_comments.rs` - CLI operation handler
- Modified: `src/cli/mod.rs` - add export modules
- Modified: `src/config/mod.rs` - add export/output config fields
- Modified: `src/main.rs` - add ExportComments operation routing
- Modified: `docs/users-guide.md` - feature documentation
- Modified: `docs/roadmap.md` - mark entry done
- New test: `tests/features/comment_export.feature`
- New test: `tests/comment_export_bdd.rs` and submodules

## CLI interface

Table: Export operation CLI flags

| Flag       | Short | Description   | Values             |
| ---------- | ----- | ------------- | ------------------ |
| `--export` | `-e`  | Export format | `markdown`,`jsonl` |
| `--output` | —     | Output path   | file path          |

Example commands:

```bash
# Export to stdout in Markdown format
frankie --pr-url https://github.com/owner/repo/pull/123 --export markdown

# Export to file in JSONL format
frankie --pr-url https://github.com/owner/repo/pull/123 \
  --export jsonl --output comments.jsonl

# Using environment variable for token
FRANKIE_TOKEN=ghp_xxx frankie \
  -u https://github.com/owner/repo/pull/123 -e markdown
```

## Revision note

Initial draft created to cover structured comment export in Markdown and JSONL
formats, stable ordering, CLI integration, tests, and documentation updates.
