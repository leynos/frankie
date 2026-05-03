# Frankie developer's guide

This guide documents internal contracts that complement the public Frankie
crate API. It targets contributors who need to extend Frankie or embed it as a
library.

## DEFAULT_REPLY_TEMPLATES

- **Location**: defined in `src/reply_template/mod.rs` and re-exported from
  the crate root as `frankie::DEFAULT_REPLY_TEMPLATES`.
- **Purpose**: single canonical source of the built-in reply-template
  defaults that drive both TUI keyboard-slot insertion and `FrankieConfig`
  defaults. Adding, removing, or reordering an entry here changes the
  defaults everywhere.
- **Ownership semantics**: the public constant is `&[&'static str]`
  (borrowed). Internal consumers that need owned values call the
  `pub(crate)` helper `default_reply_templates_owned()` to obtain a
  `Vec<String>`. The owned conversion deliberately stays inside the crate so
  external callers do not depend on Frankie's internal ownership choices.
- **Consumers**:
  - `FrankieConfig::default()` in `src/config/mod.rs` — populates the
    `reply_templates` field with the default list.
  - `ReplyDraftConfig::default()` in `src/tui/reply_draft_config.rs` —
    populates the TUI reply-draft template list.
- **Architectural boundary**: keeping `default_reply_templates_owned()` as
  `pub(crate)` while exposing only the borrowed constant publicly means
  embedding hosts must convert ownership themselves if they need it. That
  preserves Frankie's freedom to change the internal helper signature
  without breaking the public API.

## Internal API usage

Table: Internal consumers of `DEFAULT_REPLY_TEMPLATES` and the
`pub(crate)` owned-conversion helper.

| Consumer                                            | Module                          | Helper used                         |
| --------------------------------------------------- | ------------------------------- | ----------------------------------- |
| `FrankieConfig::default`                            | `src/config/mod.rs`             | `default_reply_templates_owned()`   |
| `ReplyDraftConfig::default`                         | `src/tui/reply_draft_config.rs` | `default_reply_templates_owned()`   |
| Public re-export `frankie::DEFAULT_REPLY_TEMPLATES` | `src/lib.rs`                    | none (borrowed `&[&str]` re-export) |

## Cross-references

- See `docs/frankie-design.md` (ADR-005 follow-on, roadmap step 3.2.5) for the
  cross-surface, library-first delivery rationale that motivates exposing
  `DEFAULT_REPLY_TEMPLATES` as a public library API.
