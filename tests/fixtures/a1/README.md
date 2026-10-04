# A1 synthetic inputs

`smoke.json` contains three original synthetic demonstration cases. Its expected
engineering outcomes are exercised by the example tests. It is not the A1
reference set and has no independent linguistic acceptance.

`review-draft-v2.json` contains the **frozen visible inputs evaluated on 2026-10-04**;
its historical filename is retained. Its 36 cases comprise
24 visible development candidates and 12 challenge candidates. Proposed labels,
category/family assignments, and the initial reference record are kept separately
outside Git. There are no held-out inputs or answers in this directory. The
implementer authored this draft after writing contract tests; overlapping text
and constructions are exposed development material, not blind evidence.
All 36 cases of the preserved `review-draft.json` were reviewed; one focused
clarification left two proposed core positives unresolved. Revision 2 replaces
both affected pairs before freezing, preserving family lineages and category
balance. The original file and reviewed packets remain unchanged; exclusions,
old/new mappings and initial proposals stay outside Git. The active draft retains
24 supported provisional core outcomes and 12 challenge references after the
four replacement reviews were reconciled. The
[four-case packet](../../../docs/A1_FOUR_CASE_REVIEW_PACKET.md) remains unchanged
as the reviewed snapshot; it does not need another review.
Both draft versions remain exclusion material for private holdout authoring.
Detailed reviews and adjudications stay outside Git. The reference gate/freeze
preceded the visible run; the implementation is now frozen before held-out input
release. Preserve these exact inputs and the superseded version. Do not count
the visible results as held-out acceptance; the
[evaluation status](../../../docs/A1_EVALUATION_STATUS.md) records a failed
challenge safeguard and the pending held-out run.

The draft has opaque IDs, a fixed shuffled order, and no answers, analyzer tokens,
category, partition, or performance claims. Give the reviewer the
[manual prompt](../../../docs/A1_EXTERNAL_REVIEW_PROMPT.md), the
[common binding sheet](../../../docs/A1_BLIND_PACKET_HEADER.md), and at most six
cases at a time, preserving their exact JSON. The packet is original synthetic
material, not a redacted or modified learner export. Free-form descriptions,
identifiers, permission sets, and sentences were authored for this investigation.

Authorship/provenance: Codex-assisted original authoring for Yomibu, 2026-10-03.
No learner cache, private grammar declarations, source examples, or textbook
exercises were used. These newly authored synthetic fixture texts, including
A1 test literals, are offered for unrestricted reuse under
[CC0 1.0](https://creativecommons.org/publicdomain/zero/1.0/).
This applies only to these original synthetic texts; it does not relicense
reference sources, dictionary data, existing fixtures, or application code.

The runtime JSON requires version 1, a synthetic-material attestation, explicit
reuse terms, and 1–60 cases with unique nonblank IDs. It rejects unknown fields.
A packet is bounded to 1 MiB. The attestation records the author's claim; it is
not an automated privacy, licensing, or citation audit. Each case supplies its
sentence, ordered grammar descriptions, and explicit vocabulary/grammar bindings.
No labels or source ledger enter the executable.
