# Retired generation experiments

G1 and G2 were engineering steps toward the current story path. Their production
entry points, CLI composition, authored situation selector and focused observations
are retired as of 2026-10-06. Start at
[`plan_generation` and `generate_story`](../../../crates/yomibu/src/application/story.rs) for current execution;
[story usage](../../STORY_GENERATION.md) describes its library stages and contracts.

## What the experiments established

- **G1:** one explicitly authorized provider attempt produced a fixed pair of
  original sentence candidates. Independent pinned local assessment, typed errors,
  safe reporting and bounded transport established the engineering boundaries.
  The user-run live smoke was reported as passed in [G1](G1.md); it was not a
  controlled reading-quality or accepted-exercise evaluation.
- **G2:** offline selection of one supplied lexical tuple and an authored situation
  made the exact prompt inspectable. Selection happened once; immutable request
  bytes were sent unchanged. Assessment used the full original permissions, with
  focus/context observations kept separate from conservative structural checks.
  [G2](G2.md) records the detailed historical contract and hypotheses.
- **Current story path:** manual and WaniKani inputs produce one learner inventory;
  requests supply a brief and zero or more vocabulary/grammar targets. Explicit
  cached retrieval selects prompt supports. The complete inventory still governs
  assessment. Preview and generation have separate orchestration, reporting stays
  separate, and execution options control candidate count.

The useful G1/G2 transport and report regressions now exercise the story path:
exact outgoing bytes/settings/provenance, whole-response rejection, byte caps,
deadlines, truncated/chunked bodies, redirects, credential handling, no retries,
original candidates and typed partial reports. Existing story, evaluation and
real-analyzer tests protect target uncertainty and full-inventory checks. Only
obsolete selector, fixed-pair and legacy CLI/schema tests were retired.

These tests establish engineering behavior, not adherence, naturalness, contextual
reading/sense, comprehension or enjoyment. The public synthetic G2 v1/v2 live
comparison remains **NOT RUN**; see [its recipe and manifest](G2_COMPARISON.md).
No live model calls were made for retirement and no accepted exercise is claimed.

The unused A1 packet runner is also retired; its useful protections now run through
shared story, evaluation and analyzer tests. Historical packet inputs and records
remain in [analysis history](../analysis/a1-fixtures/README.md) and these docs.

A1's frozen historical **no-go** remains: 24/24 held-out outcomes, 120/120 check
judgments and 11/12 exact negative-span matches did not outweigh three unsupported
challenge Pass results. Its later object-combination safeguard deliberately keeps
those constructions unresolved, including ordinary object sentences. Retirement
does not revise the frozen evidence or rerun private holdouts. See
[A1 evidence](../analysis/A1_IMPLEMENTATION.md) and [the safeguard](../analysis/A1_FOLLOWUP.md).

## Reproducibility without compiled legacy code

Original public synthetic inputs, exact outbound requests, SHA-256 fixtures and
comparison manifests are archived byte-for-byte outside the active test tree:

- [G1 fixtures](g1-fixtures/README.md)
- [G2 fixtures](g2-fixtures/README.md)
- [G2 comparison inputs and requests](g2-fixtures/comparison/README.md)

Code remains reproducible through Git history, rather than an unused Rust module:

| Purpose | Pinned revision |
| --- | --- |
| Last A1 packet runner (current story stages still present) | [`d2adfcfe6dffede63363bf1a11be81ce8fe85606`](https://github.com/kalorz/yomibu/tree/d2adfcfe6dffede63363bf1a11be81ce8fe85606) |
| Last pre-retirement G1 CLI and G1/G2 library APIs, alongside story | [`48825a05b7a8d84a5109b7d3be9ed3f6093f2033`](https://github.com/kalorz/yomibu/tree/48825a05b7a8d84a5109b7d3be9ed3f6093f2033) |
| G2 CLI and readable focused orchestration before story migration | [`5b9a37ab8bf7bc69427d3c8886090f76005d0236`](https://github.com/kalorz/yomibu/tree/5b9a37ab8bf7bc69427d3c8886090f76005d0236) |
| G2 comparison v1 baseline | [`818dda5e6897e4d8ab729ed9198e5070d16af50b`](https://github.com/kalorz/yomibu/tree/818dda5e6897e4d8ab729ed9198e5070d16af50b) |
| G2 comparison v2 production code | [`6c5e1343ca6583fc76211c6a757ec0070f5e7325`](https://github.com/kalorz/yomibu/tree/6c5e1343ca6583fc76211c6a757ec0070f5e7325) |

Use an isolated checkout, for example:

```sh
git worktree add --detach /tmp/yomibu-generation-history \
  48825a05b7a8d84a5109b7d3be9ed3f6093f2033
```

Historical documents describe commands/APIs at their stated pins, not current
usage. Old fixture paths still exist in those checkouts. Analyzer tests require
the real pinned dictionary; its bytes are not committed. The comparison recipe
builds its two pinned binaries and reads the archived inputs from the delivery
checkout. History or documentation does not authorize paid execution.

## Breaking Rust API changes in retirement

Removed: `generation_context`, G1 `generate_candidates`, G2
`prepare_focused_request`/`generate_focused_candidates`, their request/context/
fixed-pair result/error types and focused assessment/observation functions.
`GenerationProvenance::focused_context` is removed; current story JSON already
omitted that field. `Client::generate_story_candidates` returns `ProviderError`
directly instead of the single-purpose `GenerationError::Provider` wrapper.

Live `CandidateAssessment`, `CandidateError`, `GenerationProvenance` and
`TokenUsage` remain in `generation`. The current story payload fixture, hash,
prompt revision, provider settings, limits, reports and evaluation behavior are
unchanged. Sync, status, preparation, analysis and dictionary behavior are unchanged.
