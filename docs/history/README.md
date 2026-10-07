# History and evidence

These records describe old revisions. Use the current [README](../../README.md)
and [plan](../../PLAN.md) for today's behavior. The old plan is preserved in
[IMPLEMENTATION.md](IMPLEMENTATION.md); its "current" sections are old checkpoints.

## Delivery records

| Date / stage | Record |
| --- | --- |
| 1a–1d | [Status, sync, resilience, CI](IMPLEMENTATION.md#completed-milestone-records) |
| 2026-09-30 | [Naming](IMPLEMENTATION.md#naming-and-design-follow-up--2026-09-30) |
| 2026-10-01 | [Architecture](IMPLEMENTATION.md#architecture-migration--2026-10-01), [G0](IMPLEMENTATION.md#g0--manual-candidate-preview) |
| 2026-10-02–03 | [Preparation](IMPLEMENTATION.md#learner-constraints-and-retrieval--approved-preparation-slice) |
| 2026-10-03–04 | [A1](IMPLEMENTATION.md#a1--review-preparation-and-implementation-2026-10-03), [analyze](IMPLEMENTATION.md#offline-analyze-cli--separate-milestone-2026-10-04), [G1](IMPLEMENTATION.md#g1--experimental-single-sentence-candidates-2026-10-04) |
| 2026-10-05 | [G2](IMPLEMENTATION.md#g2--focused-experimental-context-2026-10-05), [dictionary](IMPLEMENTATION.md#managed-dictionary-loading--implementation-2026-10-05), [test timings](IMPLEMENTATION.md#sha-256-test-profile-optimization--2026-10-05) |
| 2026-10-06 | [Story](IMPLEMENTATION.md#shared-reading-implementation--2026-10-06), [workspace](IMPLEMENTATION.md#librarycli-workspace--2026-10-06), [thin CLI](IMPLEMENTATION.md#thin-cli-slice-3--shared-reports-and-executable-layout--2026-10-06) |
| 2026-10-07 | [Library navigation](IMPLEMENTATION.md#library-navigation-by-capability--2026-10-07) |

## Analysis investigation

A1's frozen result is a no-go. The current analyzer and later object-combination
safeguard remain active. The later code does not revise that score.

- [Implementation and pins](analysis/A1_IMPLEMENTATION.md), [evaluation](analysis/A1_EVALUATION_STATUS.md), [follow-up](analysis/A1_FOLLOWUP.md).
- [Reference protocol](analysis/A1_REFERENCE_PROTOCOL.md), [packet format](analysis/A1_REVIEW_PACKET.md), [bindings](analysis/A1_BLIND_PACKET_HEADER.md), [scope](analysis/A1_SCOPE_CLARIFICATION.md).
- [Review and custody packets](analysis/), [synthetic inputs](analysis/a1-fixtures/README.md).

## Retired commands and generation experiments

- [Command history](commands/COMMAND_HISTORY.md): G0 preview and source preparation.
- [Generation history](generation/GENERATION_HISTORY.md): retired APIs and code pins.
- [G1](generation/G1.md) and [G2](generation/G2.md): original experiments.
- [G2 comparison](generation/G2_COMPARISON.md): prepared but unrun; distinct from current [retrieval comparisons](../RETRIEVAL.md).

Historical commands require their pinned checkouts. Keep frozen evidence unchanged.
Routine changes need no new history file; put results in the PR or chat.
