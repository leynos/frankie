# ADR-009: review banner translation contract

## Status

Accepted (2026-03-19): Separate pull request review banner intake,
deterministic message body translation, and approval-gated rule discovery into
composable concerns, while presenting translated findings as distinct review
artefacts with persisted per-finding decision state.

## Date

2026-03-19

## Context and problem statement

Frankie needs to present raw pull request review banners from tools such as
CodeRabbit and Sourcery as actionable review artefacts inside the library, the
TUI, and the CLI. Those banners often contain many findings in one review body,
including duplicate sections, file and line references, suggested patches, and
embedded agent prompts.

The current GitHub intake path models inline review comments and issue
comments, but it does not yet expose top-level pull request review bodies as a
shared library concept. Frankie also needs a way to persist a user’s decision
to action or suppress each extracted finding, while still handling provider
format drift over time.

## Decision drivers

- Keep banner translation deterministic for known providers.
- Preserve a clean separation between transport, parsing, presentation, and
  AI-assisted discovery.
- Expose stable, reusable library contracts for TUI, CLI, and embedded hosts.
- Avoid fabricating inline GitHub review comment identifiers for banner-only
  findings.
- Support approval-gated discovery of new or drifted rule packs without
  auto-activating unverifiable output.

## Requirements

### Functional requirements

- Ingest raw pull request review bodies as first-class data.
- Translate known provider banners into structured Frankie findings.
- Let users action or suppress individual translated findings, with that
  decision persisting.
- Surface discovery proposals for unknown or drifted banners and require user
  approval before activation.
- Expose the capability through library, TUI, and CLI surfaces where
  applicable.

### Technical requirements

- Keep deterministic translation in a non-AI shared library module.
- Keep discovery in a separate, optional AI-assisted module.
- Operate extractors over a normalized Markdown AST rather than raw regex-only
  parsing.
- Store per-finding decision state in `SQLite`.
- Store approved custom rule packs in an inspectable, versioned user
  configuration file.

## Options considered

| Option                                                                                     | Sharedness | Determinism                | Drift handling                                   | Data model clarity                                         | Delivery cost                                    |
| ------------------------------------------------------------------------------------------ | ---------- | -------------------------- | ------------------------------------------------ | ---------------------------------------------------------- | ------------------------------------------------ |
| Option A: Separate intake, deterministic translation, and discovery                        | Strong     | Strong for known providers | Strong: discovery is isolated and approval-gated | Strong: banner findings stay distinct from `ReviewComment` | Moderate                                         |
| Option B: Treat banner findings as synthetic `ReviewComment` values                        | Medium     | Medium                     | Weak: synthetic IDs blur drift and identity      | Weak: semantics leak into existing comment model           | Lower initial cost                               |
| Option C: Skip deterministic translation and rely on Large Language Model (LLM) extraction | Weak       | Weak                       | Medium: model can adapt, but outcomes drift      | Medium                                                     | Lower short-term effort, higher operational risk |

_Table: Option comparison for composability, determinism, drift handling, data
model clarity, and delivery cost._

## Decision outcome / proposed direction

Adopt **Option A** with the following contract:

- Frankie introduces a raw `PullRequestReviewBanner` library model for
  top-level review bodies.
- Deterministic translation operates in a shared library module using a
  normalized Markdown AST, provider matchers, and versioned rule packs.
- Review banner handling remains separate from translation and is responsible
  for presentation, user decisions, and adapter orchestration.
- Translated findings are represented as `BannerFinding` review artefacts, not
  as synthetic `ReviewComment` values.
- Per-finding decision state is stored in `SQLite` with `pending`,
  `actioned`, and `suppressed` states.
- LLM-backed discovery emits a `ProposedRulePack`, which must pass local
  validation and explicit user approval before activation.
- Approved custom rule packs are stored in a versioned user configuration file
  rather than being hidden solely inside the database.
- Built-in rule packs are loaded first. A custom pack overrides a built-in pack
  only when it shares the same `provider_key` and has a strictly higher
  `rule_pack_version`. Custom packs with an equal or lower version are ignored
  with a logged warning.

## Goals and non-goals

- **Goals**
  - Stable library contracts for banner intake, translation, decisions, and
    discovery.
  - Deterministic translation for known providers.
  - Clear separation between deterministic and AI-assisted concerns.
  - Persistent user decisions on individual translated findings.
- **Non-goals**
  - Reconstructing missing GitHub discussion threads.
  - Posting replies or mutations directly against translated banner findings in
    the first iteration.
  - Automatically activating LLM-discovered rules.

## Migration plan

1. Add shared raw review banner models and intake support.
2. Implement deterministic translation, validation, and built-in rule packs.
3. Add banner handling, persisted decisions, and library, TUI, and CLI
   adapters.
4. Add approval-gated discovery and loading of custom rule packs.

## Known risks and limitations

- Provider drift can still temporarily degrade extraction quality until a new
  rule pack is approved.
- A single-banner discovery proposal can overfit to one format sample.
- Mixing translated findings and native review comments in one presentation
  layer requires capability-aware UI and CLI behaviour.

## Architectural rationale

1. **Separate intake** keeps GitHub transport concerns distinct from provider
   parsing.
2. **Deterministic translation** keeps known-provider behaviour stable,
   debuggable, and testable.
3. **Separate handling** avoids overloading `ReviewComment` with semantics that
   do not belong to native GitHub comments.
4. **Approval-gated discovery** allows adaptation to provider drift without
   sacrificing control or reproducibility.
5. **Split persistence** keeps mutable user workflow state in `SQLite`, while
   making approved rule packs inspectable and portable.

## References

- `docs/review-banner-translation.md`
- `docs/adr-005-cross-surface-library-first-delivery.md`
- `docs/frankie-design.md`
