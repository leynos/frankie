# Template-driven export customization

This execution plan (ExecPlan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` must be kept up to date as work
proceeds.

Status: COMPLETE

PLANS.md is not present in the repository root, so no additional plan
governance applies.

## Purpose / Big picture

Introduce template-driven export customization to the comment export pipeline.
Users can specify a Jinja2-compatible template file via `--template <PATH>` to
control the structure and content of exported comments. The implementation uses
`minijinja` for battle-tested template rendering. Success is visible when users
can export comments using custom templates with placeholders for file, line,
reviewer, and status, and unit/BDD tests cover substitution and escaping rules.

## Constraints

- New dependency: `minijinja = "2"` for Jinja2-compatible template rendering.
- Keep the model-view-update (MVU) split intact: export logic belongs in
  `src/export/`, CLI integration in `src/cli/`.
- Every new module begins with a `//!` module-level comment.
- No single file may exceed 400 lines; split into feature-focused modules if
  needed.
- Use `rstest` for unit tests and `rstest-bdd` v0.4.0 for behavioural tests.
- Template syntax uses Jinja2 conventions: `{{ variable }}` for interpolation,
  `{% for %}...{% endfor %}` for loops.
- Stable ordering: comments sorted by file_path, then line_number, then id
  (inherited from existing export).
- Any filesystem access must use capability-oriented `cap_std` APIs.
- Documentation updates must follow the en-GB style guide, wrap at 80 columns,
  and pass `make markdownlint`, `make fmt`, and `make nixie`.
- Use Makefile targets for validation (`make check-fmt`, `make lint`,
  `make test`).

## Tolerances (exception triggers)

- Scope: if implementation needs more than 10 new files or 600 net new lines,
  stop and escalate.
- Interface: if any existing public API signature must change beyond adding new
  items, stop and escalate.
- Dependencies: new dependency (`minijinja`) was pre-approved.
- Tests: if tests still fail after two fix attempts, stop and escalate with the
  latest failure output.
- Ambiguity: if the placeholder names materially affect downstream AI tool
  compatibility, stop and ask for confirmation.

## Risks

- Risk: Template syntax errors produce unclear messages. Severity: low.
  Likelihood: low. Mitigation: wrap minijinja errors with context.
- Risk: Missing placeholder values cause unexpected output. Severity: low.
  Likelihood: medium. Mitigation: use empty string for None values, document
  behaviour.
- Risk: Complex templates slow down export. Severity: low. Likelihood: low.
  Mitigation: typical PRs have <100 comments; performance is not a concern.

## Progress

- [x] Stage A: Add minijinja dependency
- [x] Stage B: Template engine wrapper
- [x] Stage C: Model updates (ExportFormat::Template)
- [x] Stage D: Configuration updates (template field)
- [x] Stage E: CLI integration
- [x] Stage F: Unit tests
- [x] Stage G: BDD tests
- [x] Stage H: Documentation
- [x] Stage I: Validation and close-out

## Surprises & discoveries

- The CLI module has a separate `src/cli/export/mod.rs` that re-exports from
  the library; needed to add `write_template` to both places.
- The existing comment_export_bdd.rs needed updating to handle the new
  `ExportFormat::Template` variant in its match statement.
- minijinja disables auto-escaping by setting a callback that returns
  `AutoEscape::None`, not by a simple boolean.

## Decision log

- Decision: Use `minijinja` crate instead of rolling custom template engine.
  Rationale: battle-tested, Jinja2-compatible, well-documented, avoids
  reinventing regex-based substitution. Date/Author: 2026-02-02, user
  suggestion.
- Decision: Derive `status` placeholder from `in_reply_to_id`: "reply" if
  present, "comment" otherwise. Rationale: provides meaningful status without
  requiring API changes; acceptance criteria mention "status" but
  `ExportedComment` has no status field. Date/Author: 2026-02-02, plan author.
- Decision: Use empty string for None/missing field values. Rationale:
  consistent with JSONL behaviour where None fields are omitted; users can use
  Jinja conditionals if needed. Date/Author: 2026-02-02, plan author.
- Decision: Disable HTML auto-escaping by default. Rationale: users control
  output format via template; automatic escaping would break non-HTML outputs.
  Date/Author: 2026-02-02, plan author.

## Outcomes & retrospective

**Completed:** 2026-02-02

All acceptance criteria met:

- Templates support placeholders for file (`{{ c.file }}`), line
  (`{{ c.line }}`), reviewer (`{{ c.reviewer }}`), and status
  (`{{ c.status }}`).
- Unit tests (rstest) cover substitution rules for all placeholders.
- Unit tests cover escaping rules (special characters, Unicode, no HTML
  escaping by default).
- Behaviour-driven development (BDD) tests (rstest-bdd) cover 7
  scenarios: simple template, document variables, file/line placeholders,
  status for replies, status for root comments, empty comments, and invalid
  syntax error.
- `make check-fmt`, `make lint`, and `make test` succeed.
- Documentation updated in `docs/users-guide.md` with template syntax,
  available variables, and example template.
- Roadmap entry marked complete in `docs/roadmap.md`.

**Lessons learned:**

- Using an established template engine (minijinja) significantly reduced
  implementation complexity compared to a custom regex-based solution.
- The Jinja2 syntax is familiar to many developers, reducing the learning curve.
- Separating template context structs (`TemplateComment`) from domain models
  (`ExportedComment`) allows clean field mapping and derived fields like
  `status`.

## Context and orientation

The export pipeline lives under `src/export/`. Existing formatters:

- `markdown.rs` — Hardcoded Markdown output
- `jsonl.rs` — JSON Lines output
- `template.rs` — New: Jinja2-compatible template output

CLI integration is in `src/cli/export_comments.rs` which dispatches to the
appropriate formatter based on `ExportFormat`.

The `ExportedComment` struct contains:

- `id: u64` — comment identifier
- `author: Option<String>` — reviewer username
- `file_path: Option<String>` — file being reviewed
- `line_number: Option<u32>` — line in diff
- `body: Option<String>` — comment text
- `diff_hunk: Option<String>` — code context
- `commit_sha: Option<String>` — commit secure hash algorithm (SHA)
- `in_reply_to_id: Option<u64>` — parent comment ID (used to derive status)
- `created_at: Option<String>` — ISO 8601 timestamp

## Plan of work

Stage A: Add minijinja dependency to Cargo.toml.

Stage B: Create template engine wrapper in `src/export/template.rs` with
`TemplateComment` struct for template context and `write_template()` function.

Stage C: Add `Template` variant to `ExportFormat` enum, update `FromStr` and
`Display` implementations.

Stage D: Add `template: Option<String>` field to `FrankieConfig` for template
file path.

Stage E: Update CLI to read template file and pass to `write_template()` when
format is Template.

Stage F: Write unit tests covering all placeholders, missing values, special
characters, and invalid syntax.

Stage G: Create BDD feature file and tests for template export scenarios.

Stage H: Update user guide with template syntax and examples.

Stage I: Run validation gates, mark roadmap entry done.

## Concrete steps

### Stage A: Add dependency

1. Add `minijinja = "2"` to `[dependencies]` in `Cargo.toml`.

### Stage B: Template engine wrapper

1. Create `src/export/template.rs` with:
   - Module-level documentation
   - `TemplateComment` struct with Serialize derive mapping ExportedComment
     fields to template-friendly names
   - `From<&ExportedComment>` implementation
   - `write_template<W: Write>()` function using minijinja

2. Create `src/export/template_tests.rs` with unit tests (linked via `#[path]`
   attribute).

3. Update `src/export/mod.rs` to include `mod template` and export
   `write_template`.

### Stage C: Model updates

1. Add `Template` variant to `ExportFormat` enum in `src/export/model.rs`.

2. Update `FromStr` to parse "template", "tpl", "jinja", "jinja2".

3. Update `Display` to show "template".

4. Update existing tests for new variant.

### Stage D: Configuration updates

1. Add `template: Option<String>` field to `FrankieConfig` in
   `src/config/mod.rs`.

2. Update `Default` implementation.

### Stage E: CLI integration

1. Update `src/cli/export/mod.rs` to re-export `write_template`.

2. Update `src/cli/export_comments.rs`:
   - Add `load_template_if_needed()` function to read template file
   - Add `read_template_file()` helper using cap_std
   - Update `write_format()` to handle Template case
   - Update `write_output()` signature to accept template content

3. Update `src/lib.rs` to export `write_template`.

### Stage F: Unit tests

Tests in `src/export/template_tests.rs`:

- `substitutes_file_placeholder` — `{{ c.file }}` works
- `substitutes_line_placeholder` — `{{ c.line }}` works
- `substitutes_reviewer_placeholder` — `{{ c.reviewer }}` works
- `status_is_comment_for_root_comments` — `{{ c.status }}` = "comment"
- `status_is_reply_for_threaded_comments` — `{{ c.status }}` = "reply"
- `substitutes_body_placeholder` — `{{ c.body }}` works
- `substitutes_context_placeholder` — `{{ c.context }}` works
- `substitutes_commit_placeholder` — `{{ c.commit }}` works
- `substitutes_timestamp_placeholder` — `{{ c.timestamp }}` works
- `substitutes_id_placeholder` — `{{ c.id }}` works
- `substitutes_reply_to_placeholder` — `{{ c.reply_to }}` works
- `substitutes_pr_url_document_variable` — `{{ pr_url }}` works
- `substitutes_generated_at_document_variable` — `{{ generated_at }}` works
- `length_filter_works` — `{{ comments | length }}` works
- `for_loop_iterates_all_comments` — `{% for %}` iteration works
- `missing_values_render_as_empty_string` — None → ""
- `handles_unicode_in_values` — Unicode preserved
- `handles_special_chars_in_body` — quotes, newlines, tabs
- `no_html_escaping_by_default` — `<script>` not escaped
- `invalid_template_syntax_returns_error` — malformed template
- `complex_template_renders_correctly` — integration test
- `empty_comments_produces_document_only` — header/footer only

### Stage G: BDD tests

1. Create `tests/features/template_export.feature` with scenarios:
   - Export with simple template renders all placeholders
   - Template with document-level variables
   - Template renders file and line placeholders
   - Status placeholder shows reply for threaded comments
   - Status placeholder shows comment for root comments
   - Empty comment list with template produces document-only output
   - Invalid template syntax produces error

2. Create `tests/template_export_bdd.rs` entry point.

3. Create `tests/template_export_bdd/mod.rs` with support module imports.

4. Create `tests/template_export_bdd/state.rs` with `TemplateExportState`.

5. Create `tests/template_export_bdd/harness.rs` with test data generators.

### Stage H: Documentation

1. Update `docs/users-guide.md`:
   - Add "Template" to export formats list
   - Add "Custom template format" section with syntax reference
   - Add placeholder table (document-level and comment-level)
   - Add example template
   - Add template errors section
   - Update CLI flags table with `--template`
   - Update environment variables table with `FRANKIE_TEMPLATE`

### Stage I: Validation and close-out

1. Run validation gates:

    ```bash
    set -o pipefail
    make check-fmt 2>&1 | tee /tmp/frankie-check-fmt.log
    make lint 2>&1 | tee /tmp/frankie-lint.log
    make test 2>&1 | tee /tmp/frankie-test.log
    ```

2. Run documentation validators:

    ```bash
    set -o pipefail
    make markdownlint 2>&1 | tee /tmp/frankie-markdownlint.log
    make fmt 2>&1 | tee /tmp/frankie-docs-fmt.log
    make nixie 2>&1 | tee /tmp/frankie-nixie.log
    ```

3. Mark roadmap entry done in `docs/roadmap.md`.

## Validation and acceptance

Acceptance is satisfied when the following are true:

- Templates support placeholders for file, line, reviewer, and status per
  acceptance criteria.
- Unit tests (rstest) cover substitution rules for all placeholders.
- Unit tests cover escaping rules (special characters, Unicode).
- BDD tests (rstest-bdd) cover happy/unhappy paths.
- `make check-fmt`, `make lint`, and `make test` succeed.
- Documentation updates pass `make markdownlint`, `make fmt`, and `make nixie`.
- Roadmap entry marked complete.

Quality criteria:

- Tests: rstest unit tests and rstest-bdd scenarios for the new behaviour.
- Lint/typecheck: `make lint` clean.
- Formatting: `make check-fmt` clean.

## Idempotence and recovery

All steps are re-runnable. If tests fail, inspect the log files under `/tmp/`,
apply fixes, and rerun the same commands. Template rendering is deterministic
given the same input.

## Artefacts and notes

Example template:

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

## Interfaces and dependencies

- New dependency: `minijinja = "2"`
- New module: `src/export/template.rs` — template engine wrapper
- New module: `src/export/template_tests.rs` — unit tests
- New test: `tests/features/template_export.feature` — BDD scenarios
- New test: `tests/template_export_bdd.rs` and submodules
- Modified: `src/export/mod.rs` — add template module
- Modified: `src/export/model.rs` — add Template variant
- Modified: `src/config/mod.rs` — add template field
- Modified: `src/cli/export_comments.rs` — template handling
- Modified: `src/cli/export/mod.rs` — re-export write_template
- Modified: `src/lib.rs` — export write_template
- Modified: `docs/users-guide.md` — feature documentation
- Modified: `docs/roadmap.md` — mark entry done

## CLI

Template export CLI flags:

| Flag         | Description                   | Values                          |
| ------------ | ----------------------------- | ------------------------------- |
| `--export`   | Export format                 | `markdown`, `jsonl`, `template` |
| `--template` | Template file path            | file path                       |
| `--output`   | Output file (default: stdout) | file path                       |

Example commands:

```bash
# Export with custom template to stdout
frankie --pr-url https://github.com/owner/repo/pull/123 --export template \
        --template my-template.j2

# Export with custom template to file
frankie --pr-url https://github.com/owner/repo/pull/123 --export template \
        --template my-template.j2 --output comments.txt
```

## Revision note

Initial draft created to cover template-driven export customization using
minijinja for Jinja2-compatible template rendering, with placeholders for file,
line, reviewer, and status.
