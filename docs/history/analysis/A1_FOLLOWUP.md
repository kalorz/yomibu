# Post-A1 object-combination safeguard

2026-10-04. This is a separate code revision using visible development inputs.
The [completed A1 investigation](A1_EVALUATION_STATUS.md) remains **no-go**:
24/24 held-out outcomes and 120/120 check judgments matched, while exact negative
reason/span agreement remains 11/12 and three visible unsupported Pass results
failed the challenge safeguard. Those historical results are not repaired here.

## Checkpoint and scope

Local checkpoint `f1237d4` on `codex/a1-offline-analysis` preserves the completed
implementation and records. All 37 staged files were reviewed and matched the
pre-follow-up snapshot; the initial regression edits were saved separately and
restored on `codex/a1-unsupported-pass` after it advanced to the checkpoint.
Existing branches remain. The checkpoint uses the already-recorded gates for
unchanged baseline code. Nothing was pushed.

The follow-up uses the real pinned analyzer and unchanged public visible inputs.
The historical status, references, review packets, dictionary/configuration pins,
and fixture bytes are preserved. No scored holdout or private reference ledger
was opened or rerun. No learner access, sync, provider calls, new review rounds,
generation, dependency changes, or dictionary downloads were involved.

## Change and tradeoff

`direct_object: true` supplies evidence about one lexical use, not its combination
with a particular object. Recognizing noun + を + a regular polite verb therefore
cannot establish that the expression is compositional. The evaluator now retains
that limitation in Particles and Scope instead of returning Pass for both.
The finding covers the object through the predicate, excluding a preceding topic
and the optional final 。. Missing permission bindings retain their Fail reasons
and original spans, with the combination limitation alongside them. Per-word
vocabulary, inflection, and nominal checks remain separately observable.

This is a conservative restriction on supported coverage. It also withholds Pass
from ordinary object sentences; it does not detect idioms, prove a particular
interpretation, or grant exceptions for familiar phrases. Other lexical/POS,
compound and contextual limitations remain. No new go decision follows.

## Red–Green–Refactor and visible verification

- **RED:** the real-adapter regression for the first exposed multiword case
  returned Pass instead of Inconclusive. The ordinary object-construction test
  failed for the same reason after its intended coverage change was made explicit.
- **GREEN:** retain the object noun alongside を and report the unresolved
  combination in Particles and Scope. Both focused tests passed.
- **REFACTOR:** reviewed naming, duplication, modelling, ownership and borrowing.
  Renamed the transitive-use predicate, used typed test input instead of loose JSON
  indexing/cloning, and kept the production change local. No new public type,
  trait or dependency was justified. The full evaluator suite passed after review.
- Extended the regression to the other two exposed failures without further
  production behavior. Added checks for unchanged topic/object permission failure
  spans and a missing polite-ending permission alongside uncertainty. The three
  focused object tests passed. Hand-counted spans in the added test were corrected
  against original text; that was test correction, not another behavioral RED.

The thin example then ran offline on the unchanged 36-case visible packet:

| Follow-up observation | Result |
| --- | --- |
| All visible outcomes | 10 Pass, 13 Fail, 13 Inconclusive |
| Three previously unsupported Pass cases | All Inconclusive |
| Ordinary visible development positives losing Pass | 2 |
| Completed checks / execution errors | 180 / 0 |

Exact echoed inputs, completed-check inventory and original UTF-8 finding spans
were verified. Detailed output stays outside Git. No private reference comparison
or new accuracy score was computed; this visible development run cannot replace
the frozen evaluation or supply unseen evidence.

## Engineering verification

On native macOS/arm64, formatting, strict locked all-targets/all-features Clippy,
and locked full tests passed using offline dependencies: 124 test entries and one
rustdoc, zero failures or ignored tests. Tests used the real local dictionary and
local HTTP mocks. [PLAN.md](../IMPLEMENTATION.md) records the exact commands and limits.
Final diff/refactor review and whitespace checks passed; no further code change
was justified. Historical evaluation, frozen public inputs/review packets and
dependency/analyzer pins match the checkpoint bytes. No hosted or Linux run is
claimed. The unchanged A1 baseline retains its previously recorded verification.

Rust note for a Ruby developer: `Option<(&Token, &Token)>` retains borrowed noun
and particle evidence without copying tokens. Enum outcomes preserve uncertainty
while the existing combination logic lets an established Fail take precedence.
