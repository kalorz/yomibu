# A1 manual review packet format

Version 0.3, used alongside the [reference protocol](A1_REFERENCE_PROTOCOL.md).
This is a human-readable form, not a runtime schema or a populated fixture.
Angle-bracketed text is a field to fill. Synthetic case creation is authorized.
Do not send this entire file to a reviewer: send only part B and the
[external-model prompt](A1_EXTERNAL_REVIEW_PROMPT.md).

## A. Custodian's private record — never in the blind packet

Keep this outside Git and the implementer's context for held-out cases.

| Field | Value to record |
| --- | --- |
| Protocol and set version | `<version; proposed/accepted status and decision date>` |
| Custodian | `<local role/name; no learner account identifiers>` |
| Case manifest | `<opaque case ID, primary category, family ID, partition>` |
| Balance | `<counts by category, positive/negative, partition>` |
| Provenance/release | `<authoring method/date; original synthetic declaration; explicit reuse terms; private/releasable/released>` |
| Freeze | `<date, input/binding/reference/prompt SHA-256 values, exact file bytes used>` |
| Exposure log | `<who/context saw inputs or labels, when; any contamination>` |
| Initial reference pass | `<proposed outcomes, claim evidence, alternatives, unedited review>` |
| Blind review | `<provider/model display name and version; date; settings or unknown; browsing status; packet/prompt hashes; unedited response>` |
| Reconciliation | `<every disagreement; source check; any unblinded clarification; retained alternatives; decision and reason>` |
| Revisions/exclusions | `<before/after version, reason, date; keep superseded records>` |
| Final reference | `<Pass/Fail/Inconclusive expectation; source-backed/model-only/disputed; decision reason; required claims>` |

Use opaque IDs without category or label prefixes. Generate byte offsets
mechanically in the later evaluation work; verify that each stored surface is
exactly the original UTF-8 slice. If a reviewer cannot compute offsets, accept
an exact quoted surface plus its occurrence number for later local conversion.
Never trust model-calculated byte positions without checking them.

## B. Blind packet — the only case input sent to the external reviewer

### Packet header

```text
Protocol: A1 reference review, <version>
Packet ID: <opaque ID>
Language of explanations: English; preserve Japanese input exactly.
Material: original synthetic evaluation inputs only; no learner material.
Reuse provenance: <authorship/method/date and explicit reuse terms>
Task: bounded reference review, not exercise acceptance.
Exact supported scope/binding-sheet version: <version and text>
Required checks for every case: <complete list and applicability rules>
Neutral source index: <source IDs, titles, URLs, sections; no case answers>
Cases in presentation order: <opaque IDs>
```

The header includes the protocol's operational boundaries, not just feature
names. Specify non-past/past polarity forms, limited particle uses, nominal
です, whole-word authorization, and treatment of unsupported constructions.
Provide a complete source index uniformly; do not provide per-case reference
rationales disguised as citation notes. No analyzer tokens or morphology appear.
Use the protocol's required inventory as the starting point. Distinguish an
examined check with no applicable occurrence from a check that was not run;
unknown applicability cannot be treated as an empty success.

### Repeat for each case

```text
Case ID: <opaque ID>
Original sentence: <unchanged supplied sentence; not created by this template>
Input identity: <UTF-8 SHA-256, Unicode scalar count, UTF-8 byte count>
Context available to the analyzer: <none, or exact synthetic context>
Intended-use request, if part of the input: <word/reading/sense; otherwise none>

Synthetic vocabulary permissions:
  <fixture lexical ID; exact written form; associated reading/sense restrictions;
   permission; no assumed equivalences; no real source/learner IDs>
Policy for unlisted words: <explicit; cannot infer permission from components>
Synthetic kanji permissions, only if relevant to a required check: <exact set>
Whole words and components: <permissions expressed independently, without
  pre-segmenting the sentence or disclosing its expected analysis>

Synthetic free-form grammar declarations:
  <fixture declaration ID; original text, retained verbatim and in order>
Explicit evaluation bindings:
  <binding ID; declaration ID; exact recognized construction; permitted forms;
   permission; scope limits; no automatic interpretation of declaration text>
Required checks/applicability: <same rule as the header; explicit exceptions>
```

Do not add category, partition, expected result, violation hints, translations of
the sentence, first-review annotations, or analyzer outputs. Include an intended
reading/sense only if it really is a request the analyzer also receives; mark it
as an intention to test, not a verified interpretation. The synthetic allowance
is fixture policy, not proof that its lexical fields are linguistically correct.

## C. Reviewer response — save unedited

For every case ask for the following record. Values are judgments to audit, not
self-certified verified references.

```text
Case ID:
Review completion: complete / partial / unable
Bounded reference recommendation: Pass / Fail / Inconclusive / none if unable
Short reason and decisive original surface/occurrence:

Required-check findings (one row per required check):
  <check ID; Pass/Fail/Inconclusive; surface and occurrence or verified span;
   claim; source ID/locator; alternative or limitation>

Lexical/morphological analysis:
  <surface + occurrence; whole lexical unit; component relationships; lemma;
   verb class/form where relevant; alternatives; evidence>
Uncovered text or unsupported construction:
  <exact surface/occurrence; reason; none only if actually reviewed>
Reading/sense alternatives:
  <interpretation; source or model-only; does it change the recommendation?>
Constraint violation versus Japanese validity:
  <state which is claimed; do not infer one from the other>

Evidence records:
  <claim ID; publisher/title; URL; entry/section/table locator; accessed or not;
   short paraphrase; direct evidence or rule application; remaining inference;
   contrary evidence/limitations; proposed source-backed or model-only status>

Unresolved questions/disagreements:
Source-access or completion problems:
```

The coordinator subsequently adds citation verification; never edit it into the
model's original answer as if the model performed that verification.

## D. Citation and disagreement ledger — local reconciliation

One row per decisive claim, with separate rows for material alternatives:

| Field | Required content |
| --- | --- |
| Case/claim | Opaque ID; original surface/occurrence; exact asserted fact |
| Source | Publisher, title, stable URL, section/entry/sense identifier, access date and source revision if shown |
| Inspection | Who/what opened it; whether full relevant text was visible; missing access is explicit |
| Entailment | What the passage supports; what it does not; direct or rule application; English explanation |
| Restrictions | Lexical class, spelling/reading/sense restriction, register, context, exception, or known source discrepancy |
| Competing judgments | Initial and blind results, every outcome-changing alternative, common upstream source if known |
| Version history | Original packet/prompt hashes; any clarification version; old and revised check judgments, with reasons; never overwrite raw reviews |
| Disposition | Source-backed/provisional/disputed; reason; unresolved question; link to preserved raw reviews |

A verified URL with an irrelevant passage is not verified evidence. If a source
is unavailable, record that; use an accessible substantiating source or leave
the claim provisional. A second model's translation is an attributed aid, not a
qualified human check. Do not repair missing evidence by majority vote.

## E. Evaluation report — only after a frozen offline run

Keep analyzer outputs separate from reference records until both are saved.

```text
Title: A1 bounded offline analysis evaluation — not accepted exercises
Run identity/date:
Protocol and reference-set versions/checksums:
Code revision; analyzer Git revision; dictionary version/checksum;
configuration checksum; C/A modes; platform/toolchain:
Blindness and freeze record; deviations:

Reference coverage (not analyzer performance):
  Source-backed / model-only / disputed / missing, by category and partition;
  direct evidence versus rule applications; unresolved alternatives;
  core gate met? Fixed planned denominators: 24 development, 24 held-out.

Analyzer results:
  Development and held-out reported separately;
  reference positive/negative rows versus Pass/Fail/Inconclusive/error/NotRun;
  correct reason/span counts; false acceptance; false rejection;
  all held-out targets, each with numerator/denominator and met/not met/not evaluable;
  all 12 challenges: supported violation or exposed limitation, unsupported passes;
  model-only exploratory tally explicitly separate; never called accuracy/truth.

Run integrity:
  Every planned case present; every required check accounted for;
  errors/NotRun recorded separately, not converted to Inconclusive;
  original bytes/spans preserved; whole words and components retained;
  output artifact identity; no after-score changes within this run.

Per-case record:
  ID; reference label + evidence status; analyzer result and per-check outcomes;
  expected/observed reason and original span; alternatives; comparison category.

Conclusion:
  bounded provisional go / analyzer no-go / evidence-limited or invalid-run no-go;
  source coverage, analyzer performance, and integrity stated separately;
  no independent linguistic-ground-truth, mastery, or exercise-acceptance claim.
Release record:
  original reusable synthetic content only; holdout scoring completed;
  private learner material and copied source exercises absent;
  raw reviews, ledgers, and detailed reports remain private unless separately authorized.
```

An empty or unrun report states **not performed**. Never populate outcome counts
from expectations or prior preparation acceptance; those measure different work.

## Runtime input and current handoff

The concrete JSON input is illustrated by
[smoke.json](a1-fixtures/smoke.json); it has no reference labels.
The [common binding sheet](A1_BLIND_PACKET_HEADER.md) maps its fields and seven
rule names to this protocol. The [active visible review draft](a1-fixtures/review-draft-v2.json)
contains original, unfrozen inputs whose reviews are complete; the
[original draft](a1-fixtures/review-draft.json) is retained for exposure
history. For new cases, send exact inputs with the common sheet and review prompt
in small batches. Keep part A, C, D, and detailed part E records private.
The [implementation status](A1_IMPLEMENTATION.md) distinguishes contract tests,
source coverage, and the still-unperformed held-out investigation.
