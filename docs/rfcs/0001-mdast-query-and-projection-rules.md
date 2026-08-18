# RFC 0001: Generic MDAST query and projection rules

## Preamble

- **RFC number:** 0001
- **Status:** Proposed
- **Created:** 2026-08-19

## Summary

Implement Frankie's deterministic review-message extraction on top of the
public generic tree interfaces in `ast-grep-core` and the serializable rule and
CSS-like selector machinery in `ast-grep-config`.

Frankie should adapt its normalized Markdown Abstract Syntax Tree (MDAST) to
`ast-grep` through private `MdastDoc`, `MdastNode`, and `MdastLanguage` types.
Provider rule packs should use a Frankie-owned, versioned schema that compiles
to `ast-grep` matchers plus a small projection and validation layer. Upstream
Rust types and serialized rule types must not become part of Frankie's public
API.

The proposal deliberately stops at **query, capture, projection, and
validation**. It does not add another parser, expose `ast-grep` source
rewriting, or attempt to build a general Tsurgeon, XSLT, or Stratego equivalent.
Frankie's provider rules remain bounded, declarative data.

The same translation pipeline should accept both top-level pull request review
bodies and pull request conversation comments. In GitHub's data model, those
conversation comments are **issue comments**, even when the issue is a pull
request. Line-anchored pull request review comments remain native Frankie
`ReviewComment` values and must not be reconstructed from banner prose.

Adoption is conditional on a small implementation spike. The spike must prove
that a non-Tree-sitter `Doc` adapter can evaluate the worked examples without
forking `ast-grep`, enabling its Tree-sitter feature, or exposing misleading
source-edit APIs.

## Problem

[ADR-009](../adr-009-review-banner-translation-contract.md) requires versioned,
serializable provider rule packs operating on a normalized Markdown AST. The
[review banner translation design](../review-banner-translation.md) defines the
pipeline and output contracts, but deliberately leaves the query and
projection mechanism unspecified.

That mechanism must express queries such as:

> Select the table after the heading containing "Pre-merge checks", except for
> rows whose second column is "Inconclusive".

Real provider messages need more than that compact example:

- ordered parent, child, ancestor, descendant, and sibling relations;
- predicates over node kinds, scalar properties, and semantic descendant text;
- negation, conjunction, disjunction, and positional selection;
- reusable named selections and projections into typed output fields;
- count, cardinality, and completeness validation;
- deterministic handling of GitHub-flavoured Markdown and normalized embedded
  HTML such as `<details>` and `<summary>`; and
- resource limits suitable for locally approved rules and untrusted discovery
  proposals.

A hand-written visitor for each provider would avoid a query parser, but would
make rule packs executable Rust rather than inspectable data. A new
Frankie-specific query language would provide control, but its lexer, parser,
precedence rules, diagnostics, tests, and compatibility policy would become a
new subsystem before the first CodeRabbit fixture translated.

The project therefore needs a smaller seam: reuse a mature structural matching
kernel and add only the Markdown-specific data model, projection, and
validation operations that kernel cannot supply.

## Current state

### Frankie intake and translation

Frankie currently distinguishes two GitHub comment forms:

- `PullRequestComment` represents a general pull request conversation comment.
  GitHub serves these through its issue-comments API because every pull request
  is also an issue.
- `ReviewComment` represents a line-anchored comment attached to a pull request
  diff.

Frankie does not yet expose top-level pull request review submissions as a
public model. Roadmap item 3.4.1 and ADR-009 introduce that intake as
`PullRequestReviewBanner` or an equivalent type. Roadmap item 3.4.2 then adds
normalized-MDAST translation and the first built-in CodeRabbit rule pack.

The three relevant GitHub objects must remain distinct:

| GitHub surface | GitHub API object | Current Frankie model | Proposed treatment |
| --- | --- | --- | --- |
| Pull request Conversation tab comment | Issue comment | `PullRequestComment` | Preserve raw data and translate when a provider rule matches |
| Submitted review summary or banner | Pull request review | Not yet modelled | Add raw review intake under ADR-009 and translate the body |
| Files changed line discussion | Pull request review comment | `ReviewComment` | Preserve as a native line-anchored artefact; optionally enrich, but do not synthesize it from a banner |

_Table 1: GitHub review-message objects and their Frankie treatment._

This distinction matters for CodeRabbit. A long Walkthrough and Pre-merge
checks block may arrive as an issue comment. Its "Actionable comments posted"
summary may arrive as a pull request review body. Individual findings normally
arrive as pull request review comments with native file and line metadata.
Provider formats can drift, so these examples describe observed shapes rather
than a permanent CodeRabbit protocol.

### Relevant `ast-grep` capabilities

The evaluated upstream release is `ast-grep` 0.45.1. Its public core defines a
generic `Doc` abstraction and an `SgNode` abstraction with parent, child,
ancestor, descendant, preceding-sibling, and following-sibling traversal. The
matcher layer operates over any `Doc`, not only the built-in Tree-sitter
implementation.

`ast-grep-config` already supplies:

- serializable atomic rules for node kind, regular expression, and sibling
  position;
- relational rules named `inside`, `has`, `precedes`, and `follows`;
- Boolean composition through `all`, `any`, and `not`;
- reusable utility rules;
- a CSS-like selector parser supporting descendant, child, adjacent-sibling,
  and general-sibling combinators; and
- `:has()`, `:not()`, `:is()`, `:nth-child()`, and
  `:nth-last-child()` pseudo-classes.

These relations cover most of the XPath-axis and Tregex-style semantics Frankie
needs. The selector parser supplies a compact unist-like surface for common
structural cases.

The upstream facilities do not solve the whole problem:

- the CSS-like parser does not currently support arbitrary attribute selectors;
- `ast-grep` source patterns assume that a language parser can parse a pattern;
- selector matches do not provide arbitrary Tregex-style named captures;
- source rewriting produces textual edits rather than mutations of an
  arbitrary MDAST value; and
- `ast-grep` does not define provider-specific projections into Frankie data
  transfer objects (DTOs).

The proposal treats those as boundaries rather than invitations to fork the
upstream parser.

## Goals and non-goals

- Goals:
  - Reuse `ast-grep`'s tested structural query parser and relation semantics.
  - Query Frankie's normalized MDAST without converting it to XML, JSON, or a
    Tree-sitter Markdown tree.
  - Keep provider rule packs serializable, inspectable, deterministic, and
    safe to validate before activation.
  - Support pull request review bodies and issue comments through one
    translation input contract.
  - Preserve native pull request review comments and use their GitHub metadata
    to prevent duplicate synthetic findings.
  - Provide semantic text, scalar-property predicates, relative projection,
    and count or cardinality validation.
  - Keep `ast-grep` behind a private adapter so upstream API changes do not
    become Frankie API changes.
  - Give rule discovery a closed target schema rather than free-form Rust or a
    Turing-complete expression language.
- Non-goals:
  - A general-purpose tree transformation language.
  - In-place MDAST mutation, source-to-source Markdown rewriting, or arbitrary
    node insertion and movement.
  - Full CSS Selectors Level 4, XPath 3.1, XQuery, XSLT, Stratego, or Tsurgeon
    compatibility.
  - Replacing GitHub's native file, line, review-thread, or reply metadata with
    values parsed from prose.
  - Exposing `ast-grep-core`, `ast-grep-config`, or their serialized rule schema
    as part of Frankie's stable public API.
  - Automatically activating an AI-discovered rule pack.
  - Choosing the Markdown parser implementation. ADR-009's normalized-MDAST
    contract remains authoritative for that decision.

## Proposed design

### Translation input envelope

Keep transport-specific raw models, then adapt translatable bodies into a
shared borrowed input:

```rust
pub enum ReviewMessageOrigin {
    IssueComment {
        comment_id: u64,
    },
    PullRequestReview {
        review_id: u64,
        state: PullRequestReviewState,
        commit_sha: Option<String>,
    },
}

pub struct TranslatableReviewMessage<'body> {
    pub origin: ReviewMessageOrigin,
    pub author_login: Option<&'body str>,
    pub body_markdown: &'body str,
    pub source_url: Option<&'body str>,
}
```

The names are illustrative rather than a final public API commitment. The
important contract is that issue comments and review bodies enter the same
parser, normalizer, classifier, and extractor after intake.

Add the parent review identifier exposed by GitHub's review-comment API to the
raw `ReviewComment` model as an optional field. A CodeRabbit review body can
then validate its declared actionable-comment count against native inline
comments with the same review identifier. The same relation prevents Frankie
from materializing an aggregate agent prompt as duplicate findings when native
comments already exist.

### MDAST adapter

Implement three private adapter types:

```rust
struct MdastDoc;
struct MdastNode;
struct MdastLanguage;
```

Their responsibilities are:

- `MdastNode: ast_grep_core::source::SgNode`
  - expose stable node identity;
  - expose ordered structural children and parent links;
  - expose kinds such as `heading`, `table`, `tableRow`, `tableCell`,
    `details`, `summary`, `code`, `paragraph`, and `htmlComment`;
  - expose source positions from the normalized tree where available; and
  - retain sibling ordering without repeated tree scans.
- `MdastDoc: ast_grep_core::Doc`
  - own or borrow the normalized tree and its node index;
  - return normalized semantic descendant text for query matching;
  - retain raw source ranges separately for evidence excerpts; and
  - reject textual edit operations through a typed query-only boundary.
- `MdastLanguage: ast_grep_core::Language`
  - map MDAST kind and field names to stable internal identifiers;
  - reject source-pattern construction because MDAST rules are not source-code
    patterns; and
  - avoid depending on Tree-sitter.

Using semantic descendant text for query matching means a heading such as
`## Pre-*merge* checks` has the string value `Pre-merge checks`, despite the
emphasis node splitting the source text. Raw Markdown remains available for
source evidence and diagnostics.

Scalar properties such as heading depth, code language, list ordering, and
link destination need one of two adapter strategies:

1. synthetic field nodes exposed through `field()` and `field_children()`; or
2. a Frankie-specific property matcher in the compiled rule layer.

The spike should evaluate both. Synthetic fields maximize reuse of upstream
relations; a property matcher may produce clearer diagnostics and fewer
fictional nodes. Either way, scalar-property syntax remains owned by Frankie.

### Rule-pack schema and compilation

The public rule-pack document remains a Frankie schema. It should contain:

- provider classification data;
- named queries;
- projections;
- validation invariants;
- ignored or metadata-only regions;
- resource budgets; and
- provider and schema versions.

Each named query compiles into an `ast-grep` matcher plus a scope. The matcher
vocabulary should initially include:

- `kind`, normalized-text `regex`, and `nthChild`;
- `inside`, `has`, `precedes`, and `follows`;
- `all`, `any`, and `not`;
- reusable utility rules;
- scalar-property equality or regular-expression predicates; and
- an optional CSS-like `selector` string compiled by `parse_selector`.

CSS-like selectors should remain structural. Frankie should not fork the
selector parser merely to add text and property syntax. Those predicates can be
composed in the structured outer rule. A selector attached to a named query is
evaluated within that query's scope. The first schema should not invent
unsupported top-level relative selectors or pseudo-classes.

Named queries provide capture semantics without pretending that source-pattern
metavariables work on MDAST. A later query can select from the result of an
earlier query. Projection expressions then select relative nodes and extract
normalized text, raw Markdown, properties, or source locations.

Illustrative shape:

```yaml
schema_version: 1
provider_key: coderabbit
rule_pack_version: 1

queries:
  failed_checks_table:
    match:
      all:
        - kind: table
        - follows:
            stop_by: neighbor
            rule:
              all:
                - kind: heading
                - property: { name: depth, equals: 3 }
                - text: { contains: "Failed checks" }

  actionable_check_rows:
    from: failed_checks_table
    select: "tableRow:not(:nth-child(1))"
    where:
      not:
        has:
          stop_by: end
          rule:
            all:
              - kind: tableCell
              - nth_child: 2
              - text:
                  regex: '^(?:❓\s*)?Inconclusive$'

emit:
  - foreach: actionable_check_rows
    type: pre_merge_check
    fields:
      name: { select: "tableCell:nth-child(1)", text: normalized }
      status: { select: "tableCell:nth-child(2)", text: normalized }
      explanation: { select: "tableCell:nth-child(3)", text: normalized }
      resolution: { select: "tableCell:nth-child(4)", text: normalized }
```

The exact YAML is proposed for discussion. The architectural boundary is the
important part: Frankie parses its outer schema with Serde, validates it,
compiles the structural subset to upstream matchers, and owns projection and
validation semantics.

### Projection and validation

Projection should remain deliberately smaller than a general expression
language. Initial operations should include:

- select the first, all, or exactly one relative match;
- return normalized semantic text or raw source text;
- read a scalar MDAST property;
- apply trim and whitespace normalization;
- capture a regular-expression group from normalized text;
- parse a bounded unsigned integer;
- construct a typed enum through an explicit mapping table; and
- construct output records from named values.

Validation should include:

- required, optional, minimum, maximum, and exact cardinality;
- equality between a parsed provider count and an extracted or native count;
- non-overlapping source regions where required;
- required output fields;
- unique stable item keys; and
- complete, partial, drifted, and failed outcomes.

These operations are sufficient for the first CodeRabbit pack and remain easy
to validate before an AI-discovered rule becomes eligible for approval.

### Correlation with native review comments

Translation must not make a worse copy of data GitHub already provides.

For each pull request review body, Frankie should load native review comments
and group them by `pull_request_review_id`. A provider rule may then:

- validate a declared actionable count against the native group;
- attach review-level metadata or an aggregate agent prompt to that group;
- emit a synthetic finding only when the provider describes an actionable item
  for which no native review comment exists; and
- report a drift warning when the declared and native counts disagree.

Content fingerprints are a fallback only. Native identifiers, review
identifiers, file paths, lines, and provider item keys take precedence over
prose similarity.

### Safety and resource governance

Provider Markdown and embedded agent prompts are untrusted review data. Query
matching treats their contents as opaque text. It must not execute a code block,
follow instructions inside it, interpolate it into a rule, or submit it to an
agent without a separate explicit user action.

Rule validation should reject or bound:

- excessive nesting or utility-rule recursion;
- cyclic utility references;
- invalid or expensive regular expressions;
- excessive named-query counts;
- unbounded result materialization;
- unsupported source patterns or rewrite operations; and
- unknown output fields or projection functions.

Evaluation should impose document-node, match-count, elapsed-time, and
allocation budgets. A budget breach produces a structured validation failure,
not a partial success silently presented as complete.

## Worked examples

The examples below are abridged from observed CodeRabbit output on
`leynos/netsuke#553`. They are fixtures, not claims about a stable provider
protocol.

### Example 1: Pre-merge checks in an issue comment

GitHub classifies the large Conversation-tab message as an issue comment. An
abridged body is:

```markdown
## Walkthrough

The runner now derives an explicit policy from CLI state.

### ❌ Failed checks (2 warnings, 2 inconclusive)

| Check name | Status | Explanation | Resolution |
| --- | --- | --- | --- |
| Out of Scope Changes check | ⚠️ Warning | The PR also extracts reporter construction. | Remove or justify the extra change. |
| User-Facing Documentation | ⚠️ Warning | The public examples are stale. | Update the users' guide. |
| Testing (Overall) | ❓ Inconclusive | Investigation started. | Inspect the tests. |
| Observability | ❓ Inconclusive | The complete diff is unavailable. | Provide the complete diff. |

<details>
<summary>✅ Passed checks</summary>

| Check name | Status | Explanation |
| --- | --- | --- |
| Title check | ✅ Passed | The title describes the change. |
</details>
```

The illustrative rule above identifies the table by its adjacent heading,
selects non-header rows, and excludes rows whose second cell normalizes to
`Inconclusive`. Projection produces:

```json
[
  {
    "type": "pre_merge_check",
    "name": "Out of Scope Changes check",
    "status": "Warning",
    "explanation": "The PR also extracts reporter construction.",
    "resolution": "Remove or justify the extra change."
  },
  {
    "type": "pre_merge_check",
    "name": "User-Facing Documentation",
    "status": "Warning",
    "explanation": "The public examples are stale.",
    "resolution": "Update the users' guide."
  }
]
```

The two inconclusive rows are absent. The passed-check table is also absent
because its preceding heading does not match the named query. A provider pack
may preserve both regions as metadata without presenting them as actionable
findings.

Validation can parse the heading counts and check that two selected rows are
warnings and two rejected rows are inconclusive. A mismatch marks the result
as drifted rather than quietly dropping rows.

### Example 2: Review body correlated with native comments

CodeRabbit may submit a pull request review body shaped like:

````markdown
**Actionable comments posted: 1**

<details>
<summary>🤖 Prompt for all review comments with AI agents</summary>

```text
Verify each finding against current code. Fix only still-valid issues.

Inline comments:
In `@docs/developers-guide.md`:
- Around line 3195-3198: Wrap the prose at 80 columns.
```

</details>
````

The review body contains useful review-level metadata and an aggregate prompt,
but the actionable finding also exists as a native pull request review comment.
An illustrative rule can capture the count and prompt without emitting a
duplicate finding:

```yaml
queries:
  declared_actionable_count:
    match:
      all:
        - kind: paragraph
        - text:
            regex: '^Actionable comments posted:\s+(?P<count>[0-9]+)$'
    project:
      count: { regex_group: count, parse: u32 }

  aggregate_agent_prompt:
    match:
      all:
        - kind: details
        - has:
            stop_by: end
            rule:
              all:
                - kind: summary
                - text: { contains: "Prompt for all review comments" }
    project:
      prompt:
        select: "code"
        text: raw

validations:
  - equals:
      left: { query: declared_actionable_count, field: count }
      right: { context: native_review_comment_count }

emit: []
```

Given review identifier `4892926770` and one native review comment carrying the
same parent review identifier, the result is review metadata plus a successful
count validation. It creates no second `BannerFinding` for the prose copy.

If GitHub returns no native comment but the review body declares one actionable
item, the provider pack may emit a review-level synthetic finding only when it
can extract the required location and identity fields with sufficient
confidence. Otherwise it reports partial or drifted translation.

### Example 3: Native line-level review comment

The related native review comment has data conceptually equivalent to:

```json
{
  "id": 3745783871,
  "pull_request_review_id": 4892926770,
  "path": "docs/developers-guide.md",
  "line": 3198,
  "body": "Wrap the changed prose at 80 columns."
}
```

Frankie should retain this as a native `ReviewComment`, including the file,
line, diff hunk, commit, thread, and reply metadata available from GitHub. The
provider translator may enrich it with a normalized category or prompt from
the review body, but must not replace its native identity or anchor with values
parsed from Markdown.

This gives the three inputs distinct outcomes:

| Input | Output |
| --- | --- |
| Issue-comment Pre-merge checks table | Structured check artefacts selected by status |
| Pull request review body | Review-level metadata, prompt, count validation, and only genuinely missing synthetic findings |
| Pull request review comment | Native line-anchored `ReviewComment`, optionally enriched |

_Table 2: Worked-example inputs and outputs._

## Requirements

### Functional requirements

- Translate provider-formatted issue comments as well as pull request review
  bodies.
- Preserve raw inputs and source positions for evidence and debugging.
- Select nodes through ordered tree relations and semantic-text predicates.
- Project selected nodes into typed Frankie output records.
- Correlate review bodies with native review comments before emitting synthetic
  findings.
- Validate provider-declared counts and required fields.
- Return explicit non-match, complete, partial, drifted, budget-exceeded, and
  failed outcomes.
- Keep result ordering stable and derived from document order unless a rule
  declares another deterministic order.

### Technical requirements

- Compile with `ast-grep-core` and `ast-grep-config` while disabling their
  Tree-sitter features for this subsystem.
- Keep all `ast-grep` types private to the translation implementation.
- Pin an evaluated upstream version and review API changes deliberately during
  dependency upgrades.
- Reject `ast-grep` source patterns and rewrite operations for `MdastDoc`.
- Validate rule depth, utility-rule cycles, regular expressions, fields,
  projections, cardinality, and resource budgets before activation.
- Make normalized semantic text and raw source excerpts separately available.
- Cover the adapter, query compiler, projections, and provider packs with unit,
  property, and golden-fixture tests.
- Record enough match evidence to explain why a rule matched or failed without
  logging private review bodies by default.

### Spike acceptance criteria

Before adding the dependency to production translation code, a disposable or
feature-gated spike must demonstrate all of the following:

1. An in-memory MDAST fixture implements `Doc`, `SgNode`, and `Language` without
   Tree-sitter.
2. `parse_selector` evaluates child, descendant, adjacent-sibling,
   general-sibling, `:has()`, `:not()`, and `:nth-child()` cases on that
   fixture.
3. The structured matcher evaluates `inside`, `has`, `precedes`, `follows`,
   Boolean composition, regular-expression text predicates, and scalar
   properties.
4. The two worked CodeRabbit examples produce the expected projections and
   reject the inconclusive rows.
5. `cargo tree` shows no Tree-sitter dependency introduced through this path.
6. Attempted source-pattern or rewrite use fails through a deliberate typed
   boundary rather than panicking or silently editing the wrong text.
7. A representative large banner stays within a documented node, time, and
   allocation budget.
8. Upstream public API usage remains confined to one private adapter module and
   one matcher compiler module.

If the spike requires a fork, relies on undocumented invariants, or cannot make
query-only text semantics unambiguous, this RFC returns to review before
implementation proceeds.

## Compatibility and migration

This proposal changes no existing user-visible behaviour until a provider rule
pack is enabled.

Implementation should proceed in the following order:

1. Extend raw GitHub intake with top-level pull request reviews and the optional
   parent review identifier on native review comments.
2. Establish complete, redacted CodeRabbit fixtures for one issue comment, one
   pull request review body, and their correlated native review comments.
3. Complete the `ast-grep` MDAST adapter spike and record its result in the
   implementation pull request or execution plan.
4. Introduce the private compiled matcher and Frankie-owned serialized rule
   schema.
5. Implement query, projection, and validation for the worked examples.
6. Add the built-in CodeRabbit pack behind the existing translation API.
7. Add Terminal User Interface (TUI) and Command Line Interface (CLI)
   presentation only after the shared library output and persistence contracts
   are stable.
8. Enable approval-gated discovery against the same schema after built-in packs
   establish a useful fixture corpus.

Existing approved custom rule packs do not yet exist, so the first schema can
start at version 1. Once published, schema changes must follow an explicit
compatibility policy. Provider rule-pack versions remain independent of the
rule-schema version.

If accepted, the roadmap should clarify that item 3.4.2 covers matching
issue-comment bodies as well as pull request review bodies when a provider emits
structured banner material through both GitHub object types.

## Alternatives considered

### Option A: Hand-written Rust extractors per provider

Implement each CodeRabbit and Sourcery format as ordinary visitors over MDAST.
This has the smallest dependency footprint and the clearest type checking. It
is a strong fallback if the adapter spike fails.

It does not satisfy the existing rule-discovery direction well. A discovered
rule would need code generation or an interpreter added later, and provider
logic would become scattered imperative traversal code. It also repeats
ancestor, sibling, text, positional, and projection mechanics in each
extractor.

### Option B: Project MDAST into XML and use XPath or XSLT

XPath offers mature axes and predicates, and XSLT adds transformation. The
projection creates a second tree model, property encoding, and source-location
mapping. Frankie's normalized MDAST and embedded HTML semantics would still
need a custom mapping.

This option solves the query language by introducing a data-model translation
layer that the proposed adapter avoids.

### Option C: Serialize MDAST and use `jaq`, JSONPath, or JSONata

A JSON query language makes property access and projection convenient. Parent
and sibling relationships become path and array-index calculations rather than
first-class axes. General-purpose expression languages also widen the resource
and validation surface for AI-discovered rules.

`jaq` remains a useful diagnostic and prototyping tool, but is not the
recommended production rule evaluator for ordered tree relations.

### Option D: Query a Tree-sitter Markdown tree with `ast-grep`

This would use `ast-grep` through its intended source-code path and provide
source patterns and replacements. It would introduce another Markdown tree
whose shape differs from the normalized MDAST contract in ADR-009. Embedded
HTML normalization, GitHub-flavoured tables, semantic text, and provider
projections would still need adaptation, while the translation pipeline would
carry two syntax trees.

### Option E: Implement a Frankie-specific query parser

A bespoke language could combine CSS selectors, XPath axes, Tregex captures,
and Tsurgeon operations exactly. It would also make Frankie responsible for
lexing, parsing, precedence, diagnostics, compatibility, editor tooling,
fuzzing, security review, and a complete semantic test suite.

The current requirements do not justify that subsystem. This option should be
reconsidered only if real provider fixtures repeatedly exceed the bounded rule
and projection schema after the `ast-grep` adapter ships.

## Open questions

- Should scalar MDAST properties use synthetic field nodes, a Frankie-specific
  matcher, or both? The spike should compare implementation clarity and
  diagnostics.
- Should `Doc::get_node_text` return semantic text directly, or should Frankie
  provide a separate semantic-text matcher and leave `Node::text()` raw? The
  query-only boundary must make either choice explicit.
- Does the first public rule-pack schema expose CSS-like `selector` strings, the
  structured matcher form, or both? Supporting both improves ergonomics but
  increases canonicalization and diagnostics work.
- Should issue-comment pre-merge checks become `BannerFinding` values or a
  broader translated-review-item type with distinct finding, check, status,
  and prompt variants?
- What fingerprint best correlates aggregate review-body prompt entries with
  native inline review comments when the provider omits or changes a path or
  line?
- Which Markdown parser and normalized-tree representation should own source
  positions, embedded HTML normalization, and semantic-text calculation?
- Should Frankie contribute attribute selectors or generic capture support
  upstream after proving the need locally, rather than extending its own
  surface?

## Recommendation

Approve the query-only direction subject to the spike acceptance criteria. Use
`ast-grep` as a private structural matching kernel, not as Frankie's public
rule language and not as a Markdown rewriting engine. Keep the provider rule
pack declarative through Serde, add a small Frankie projection and validation
layer, and exercise it first against correlated CodeRabbit issue-comment,
review-body, and native review-comment fixtures.

This route reuses the difficult, well-tested parts of tree querying while
leaving Frankie responsible only for the parts genuinely specific to review
message translation: normalized MDAST semantics, GitHub object correlation,
provider projections, validation, and presentation. The Dragon Book remains a
reference rather than a project dependency.

## References

- [ADR-009: review banner translation contract][adr-009]
- [Review banner translation design][banner-design]
- [Frankie roadmap, section 3.4][roadmap-3-4]
- [ast-grep repository][ast-grep]
- [ast-grep generic source traits][ast-grep-source]
- [ast-grep relational rules][ast-grep-relations]
- [ast-grep CSS-like selector parser][ast-grep-selector]
- [CodeRabbit Walkthrough issue comment on netsuke#553][coderabbit-walkthrough]
- [CodeRabbit review body on netsuke#553][coderabbit-review]
- [CodeRabbit inline review comment on netsuke#553][coderabbit-inline]

[adr-009]: ../adr-009-review-banner-translation-contract.md
[banner-design]: ../review-banner-translation.md
[roadmap-3-4]: ../roadmap.md#34-review-banner-translation
[ast-grep]: https://github.com/ast-grep/ast-grep
[ast-grep-source]: https://github.com/ast-grep/ast-grep/blob/main/crates/core/src/source.rs
[ast-grep-relations]: https://github.com/ast-grep/ast-grep/blob/main/crates/config/src/rule/relational_rule.rs
[ast-grep-selector]: https://github.com/ast-grep/ast-grep/blob/main/crates/config/src/rule/selector.rs
[coderabbit-walkthrough]: https://github.com/leynos/netsuke/pull/553#issuecomment-5232992157
[coderabbit-review]: https://github.com/leynos/netsuke/pull/553#pullrequestreview-4892926770
[coderabbit-inline]: https://github.com/leynos/netsuke/pull/553#discussion_r3745783871
