# Architectural decision record (ADR) 006: AI rewrite preview and fallback contract

## Status

Accepted.

## Date

2026-03-02.

## Context and problem statement

Reply drafting supports AI-assisted expansion and rewording. The roadmap
acceptance criteria require explicit AI provenance, preview-before-apply, and
graceful failure handling across both Terminal User Interface (TUI) and Command
Line Interface (CLI) surfaces.

The AI integration must be testable without network access and must preserve
the original draft text when an AI request fails or returns unusable output.

## Decision outcome / proposed direction

Introduce a shared AI rewrite library boundary in `src/ai/comment_rewrite/`
with:

- `CommentRewriteMode`, `CommentRewriteRequest`, and
  `CommentRewriteOutcome::{Generated,Fallback}`.
- A reusable `CommentRewriteService` trait with an OpenAI-compatible adapter.
- A shared side-by-side preview model used by both TUI and CLI adapters.
- A mandatory `AI-originated` provenance label on generated candidates.

In the TUI, rewrite requests are asynchronous and always shown in a preview
before being applied. In CLI mode, generated and fallback outcomes are rendered
using the same shared outcome contract.

## Rationale

1. Parity across surfaces: one domain contract keeps TUI and CLI behaviour
   consistent.
2. Safety by default: preview-before-apply avoids silent draft mutation.
3. Failure resilience: fallback outcomes preserve original text and surface
   actionable reasons instead of failing the workflow.
4. Testability: a dependency-injected service trait enables deterministic unit
   tests and `rstest-bdd` behavioural scenarios.

## Consequences

- AI requests produce a preview that can be applied or discarded.
- Applied AI text is visibly labelled `AI-originated` until manually edited.
- CLI users can run non-interactive AI rewrite with generated or fallback
  output and preview metadata.
- Operation-mode precedence includes `AiRewrite` ahead of export and pull
  request loading modes when rewrite flags are set.

## References

- `docs/frankie-design.md` §5.3.6 (ADR index)
- `src/ai/comment_rewrite/` (domain and adapter boundary)
