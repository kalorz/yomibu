# Retired manual preview and source preparation

As of 2026-10-06, `preview` and `prepare`, their library entry points, standalone
grammar-file adapter and examples are retired. They were early plumbing and
source-inspection slices, independent of current story generation.

- G0 preview selected the first N supplied word tuples and checked membership/count.
  It exercised an explicit data flow, not a replaceable model plugin.
- Preparation inspected exact cached WaniKani uses and attached source examples.
  Its output was never consumed by current story generation.
- Current `preview-story` exposes the exact offline request; `prepare-retrieval`
  builds the embeddings used by the current path.
- Eligibility rules and retained source evidence still serve inventory projection;
  their tests now live in `crates/yomibu/tests/knowledge.rs` without grammar input.
- Source alternatives, original observations, terminal safety and offline behavior
  remain protected by current inventory, story, source and CLI tests. Obsolete
  first-N/tuple-inspection contracts are not retained as artificial current tests.

The last complete implementation is pinned at
[`4df4e0c2bf76cd88938488a7ea8f12a48eec89b8`](https://github.com/kalorz/yomibu/tree/4df4e0c2bf76cd88938488a7ea8f12a48eec89b8).
The synthetic grammar input is archived byte-for-byte at
[`history/commands/grammar.json`](history/commands/grammar.json).
Use the pinned checkout for historical commands/examples. No historical accuracy or
exercise-acceptance claim is added. The original contract below is historical.

### Manual candidate preview (implemented G0)

Before integrating linguistic analysis or an LLM, G0 provides a synchronous library
operation and a thin `yomibu preview` command that select the first N supplied
word entries. This is a real deterministic preview, not an accepted Japanese
exercise or an automatic fallback for another generator.

```sh
yomibu preview \
  --word '猫:ねこ:cat' \
  --word '犬:いぬ:dog' \
  --word '学校:がっこう:school' \
  --grammar 'です' \
  --grammar 'は' \
  --take 2
```

- Each repeatable `--word` value contains text, reading, and meaning. Split only
  at the first two ASCII colons; further colons belong to the meaning. Trim field
  boundaries, preserve internal content, and reject missing or blank fields.
  This shorthand does not support colons in the text or reading fields.
- The library receives structured entries, not CLI-delimited strings. Keep the
  three fields associated: another reading or meaning of the same written form
  is not implicitly supplied or allowed. These are user declarations, not verified
  dictionary facts. Do not infer other readings, synonyms, inflections, or mastery.
- `--take` is required and positive, and cannot exceed the supplied entry count.
  Preserve entry order and duplicates; select the first N entries, retaining
  text, reading, and meaning. The count concerns entries, not unique words or
  generation candidates. Invalid input produces a typed library error and a
  nonzero CLI exit with a useful explanation, rather than partial success.
- Repeatable `--grammar` values are nonblank manual descriptions. Retain them
  with the preview inputs and explicitly report grammar as not assessed. No
  grammar matcher, provider mapping, durable identity, or learner registration
  is required for these transient inputs.
- Return selected entries and an explicit report checking membership in the
  supplied entries and the requested count. Run these checks on the produced
  result; do not have the selector assert its own success. Readings, meanings,
  naturalness, and grammar are not linguistically verified by this slice.
- The CLI renders each selected word entry and grammar description on one line,
  using Rust's `str::escape_debug` for each field. Control characters, line
  separators, backslashes, and quotes are displayed as escapes so declarations
  cannot introduce extra report lines or terminal control sequences. Ordinary
  Japanese text remains readable; the underlying structured values are unchanged.
  The CLI reports the limited assessment. The direct library call exposes
  equivalent data without parsing arguments or printing.
  Preview does not resolve HOME/data directories, require tokens, create files,
  open stores, synchronize, start an async runtime, or call any service/model.

The public `preview::preview` function accepts word/grammar slices and a `usize`
count, returning `Result<Preview<'_>, PreviewError>`.
`WordEntry` has associated `text`, `reading`, and `meaning` strings. All supplied
entries and grammar descriptions are validated before selection, including the
unselected suffix. Structured library inputs are retained verbatim; CLI word
field trimming happens only at the executable boundary. Grammar descriptions
retain their content, order, and duplicates. No grammar input is also valid.

`Preview` borrows the selected entries and grammar descriptions. Its
`PreviewChecks` reports membership and count as `CheckOutcome::Pass` or `Fail`,
and grammar/linguistic correctness as `NotAssessed`. Membership compares the
complete text/reading/meaning entry, with no normalization or inferred lexical
equivalence. A temporary borrowed hash set indexes supplied entries for membership
checks; it does not deduplicate or reorder the selected output. A separate private
function checks the produced selection; its tests deliberately supply invalid
selections. Invalid input returns `PreviewError::InvalidCount`, `BlankWordField`,
or `BlankGrammar`; field errors
identify the one-based input entry. There is no partial result on input failure.

Run the synchronous direct-library example with
`cargo run --locked --example preview`; see [historical preview example](https://github.com/kalorz/yomibu/blob/4df4e0c2bf76cd88938488a7ea8f12a48eec89b8/crates/yomibu/examples/preview.rs).
The CLI's global `--data-dir` option is ignored for preview. Sync/status/prepare
resolve that option or the HOME default.

This slice's detailed delivery and acceptance criteria are in `PLAN.md` under
**G0 — Manual candidate preview**. File/stdin imports, alternate delimiters,
JSON input, Japanese text generation, model adapters, and a plugin host are
outside G0. Existing sync/status and schema-1 persistence retain their contracts.

## Learner constraints and retrieval — approved preparation slice

This slice implements synchronous, offline practice-context preparation through
`preparation::prepare_context` and a thin `yomibu prepare` command. Its result is
retrieval evidence, not an accepted exercise.

Read one coherent schema-1 WaniKani cache and one explicit grammar input. Preserve
the source observations and manual declarations; derive `LearnerKnowledge` on
demand with an explicit `LearnerKnowledgePolicy`. Never persist the derived
classification or alter synchronization/status behavior.

The default `lesson-started` rule requires a recorded assignment `started_at`;
the alternative `recorded-pass` rule requires `passed_at`. Both exclude
unavailable material and subjects hidden in any retained subject, assignment, or
review-statistic record. Review-only material is not eligible. No SRS-stage,
accuracy, recency, or linguistic-mastery threshold is implied. Eligibility is a
source-subject decision, not proof of knowledge of every reading or sense.
Report the selected policy, synchronization interval, source evidence, and
inclusion/exclusion reasons. Unavailable content has only its retained identifier
and kind; do not invent its spelling.
For each decision, the CLI shows content availability and `hidden_at`, assignment
identity/hidden state and `started_at`/`passed_at`, and review-statistic
identity/hidden state. Missing records remain explicit, including evidence that
did not determine the highest-precedence exclusion.

Grammar input is a separate local JSON document:
`{"version":1,"declarations":["です","は as a topic marker"]}`.
Each entry asserts learner familiarity for practice. Preserve descriptions
verbatim and keep duplicates independent. Assign one-based technical entry IDs
scoped to the loaded input; there is no cross-edit identity promise or write-back.
Empty declarations are valid. Reject blank descriptions, malformed input, and
unsupported versions. The file preserves learner assertions, not policy-derived
knowledge. No grammar recognizer or provider equivalence is implied.

Each repeatable `--target WORD:READING:SENSE` preserves one intended use. The CLI
splits at the first two ASCII colons and trims field boundaries; library inputs
are structured and retained verbatim. Here SENSE must match an exact cached
accepted gloss, and READING an exact cached accepted reading. No paraphrase,
normalization, inferred reading, or reading/gloss Cartesian product is supported.
A request never declares a word known. Detect multiple exact lexical matches
before applying policy; eligibility cannot disambiguate records. Resolve only
eligible vocabulary records; kanji knowledge cannot establish vocabulary
knowledge. Missing, ambiguous,
unsupported, or policy-ineligible targets fail explicitly without partial success.
Kana-only records retain their lack of source readings; do not fabricate one.

Return the selected subjects' readings, meanings, answer flags, parts of speech,
and all attached examples in source order. Keep target request order and
duplicates. Matching separate source fields does not verify their association:
reading/sense correctness and example suitability remain unassessed. Examples
are selected by subject attachment, not by proven correspondence to the intended
use or learner constraints. Missing examples are explicit empty results; there
is no synthetic or network fallback.

The preparation path requires no token, HTTP client, async runtime, implicit sync,
or writes. The CLI owns argument/environment handling and escaped presentation;
the library accepts existing values without requiring a store. The existing
`LearningStore::load` supports file/memory composition. No new trait, generator
substitution, plugin host, model integration, linguistic analysis, external
dictionary, or later milestone is included.

`GrammarDeclarations::from_descriptions` creates validated manual inputs;
`adapters::grammar_file::{load, parse}` provides the explicit read boundary.
`LearnerKnowledgePolicy::derive` returns all decisions in subject-ID order with
borrowed material, assignment, and review-statistic evidence.
`prepare_context(source, grammar, policy, targets)` returns a borrowed
`PreparedContext` or a typed `PrepareError`. The result preserves policy, learner
ID, synchronization interval, grammar assertions, and target/source associations.
Exclusion precedence is unavailable content, hidden evidence, no assignment,
then the selected missing lifecycle timestamp. Other evidence remains available.
See the [historical README](https://github.com/kalorz/yomibu/blob/4df4e0c2bf76cd88938488a7ea8f12a48eec89b8/README.md)
and [prepare example](https://github.com/kalorz/yomibu/blob/4df4e0c2bf76cd88938488a7ea8f12a48eec89b8/crates/yomibu/examples/prepare.rs)
for the retired CLI and direct-library usage.

Implementation and real-learner acceptance of this preparation slice are complete
as of 2026-10-03; `PLAN.md` records the user's completed acceptance evidence.
Eligibility remains distinct from mastery, and linguistic validity is unassessed.
