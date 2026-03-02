# Architectural decision record (ADR) 004: inline template-based reply drafting

## Status

Accepted.

## Date

2026-03-02.

## Context and problem statement

Review workflows need fast, keyboard-driven reply composition without leaving
the Terminal User Interface (TUI). The roadmap acceptance criteria require
inline rendering, edit-before-send behaviour, and configurable length
enforcement.

The initial scope must focus on drafting user experience (UX) and not expand to
live GitHub submission in the same slice.

## Decision outcome / proposed direction

Add a dedicated reply-draft state slice to the review TUI with template
insertion (`1` to `9`), free-form editing, and a local send-intent action.
Templates are rendered with `MiniJinja` using comment-scoped variables
(`comment_id`, `reviewer`, `file`, `line`, `body`).

The first-use flow is explicit: press `a` to enter reply-draft mode, use
template slots `1` to `9` to insert a starter reply, edit inline, then press
`Enter` to mark the reply-draft ready to send.

## Rationale

1. MVU boundary clarity: reply drafting is implemented as its own message group
   and handlers so navigation, Codex execution, and sync logic remain isolated.
2. Keyboard-first interaction: starting reply-draft mode with `a`, using
   template slots `1` to `9`, and confirming readiness with `Enter` aligns with
   the existing terminal-first UX and avoids modal forms.
3. Deterministic validation: draft limits are enforced as Unicode scalar counts
   during both typing and template insertion, producing consistent behaviour
   across multilingual text.
4. Scoped delivery: `Enter` marks a reply-draft as ready to send but does not
   post to GitHub in this phase.

## Consequences

- Selected comments show inline reply-draft content and draft metadata in the
  detail pane.
- Templates can be configured through config layers (`reply_max_length`,
  `reply_templates`) without code changes.
- Over-limit insertions and invalid template slots surface explicit inline
  errors instead of silently truncating content.
- Continuous Integration (CI) coverage includes the first-use reply-draft flow
  (`a`, template slots `1` to `9`, inline editing, `Enter` readiness).

## References

- `docs/frankie-design.md` §5.3.6 (ADR index)
- `src/tui/state/reply_draft.rs` (reply draft state and template rendering)
- `src/config/` (reply template configuration)
