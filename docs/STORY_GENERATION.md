# Shared story generation

The current experimental path accepts either manual material or an explicit
WaniKani cache. Source conversion ends at `LearnerInventory`; both sources use
the same request, selection, generation and assessment code. Sync, status and
supplied-text analysis are separate use cases. See the
[module map](../ARCHITECTURE.md#current-module-map) for source navigation and the
[glossary](GLOSSARY.md) for terms.

"Story" includes scenes and dialogues. Current generation produces single sentences.

## Inputs and meaning

Use the complete [inventory](../tests/fixtures/story/inventory.json) and
[request](../tests/fixtures/story/request.json) examples together.

- The inventory contains all allowed material. WaniKani eligibility uses explicit
  `lesson-started` or `recorded-pass` policy; eligibility does not prove mastery.
- The request contains a free-form `brief` and vocabulary/grammar target IDs.
  Targets must exist in the inventory and grant no new permissions. Empty lists are valid.
- Selection is the vocabulary sent in the prompt. Assessment uses the full inventory.
  The provider receives no whole account, progress history or unselected vocabulary.

IDs are stable strings. WaniKani entries use `wanikani:<subject-id>`.
Manual input can supplement a WaniKani projection, particularly its grammar;
duplicate IDs are rejected rather than silently overriding entries. A grammar
description records familiarity. A binding names an explicit
[supported rule](ANALYSIS.md#input-and-supported-checks); descriptions never create rules.

Readings and meanings remain separate source alternatives, never a Cartesian
product of asserted word uses. WaniKani accepted-answer alternatives are kept;
kana-only records keep missing readings rather than inventing them. Raw readings
are preserved, with a hiragana-to-katakana comparison form for Sudachi.
`direct_object: null` means unknown; WaniKani glosses/POS do not infer transitivity.
A single reading and meaning still does not establish contextual correctness.
The original source cache is unchanged and remains the evidence record; the
projection retains source subject IDs and exclusions.

## Commands

From the repository root, explicitly prepare an offline **lexical baseline**:

```sh
cargo run --locked -- prepare-retrieval \
  --inventory tests/fixtures/story/inventory.json \
  --request tests/fixtures/story/request.json \
  --embedding-cache /tmp/yomibu-vectors.json \
  --embedding-provider lexical-baseline

cargo run --locked -- preview-story \
  --inventory tests/fixtures/story/inventory.json \
  --request tests/fixtures/story/request.json \
  --embedding-cache /tmp/yomibu-vectors.json --json
```

The baseline is token overlap in hashed vectors, **not a semantic model**. It is
never an automatic fallback. See [retrieval evidence](RETRIEVAL.md).

Replace `--inventory PATH` with `--wanikani-cache PATH` to read a saved schema-1
WaniKani cache, optionally adding `--inventory PATH` for manual material/grammar.
`--knowledge-policy recorded-pass` changes eligibility. Story commands ignore
`--data-dir`; paths are explicit and no implicit sync occurs.

Preview and generation accept `--candidates N` (positive integer, default 2).
This belongs to execution options, not the story-request JSON. The effective
options appear separately as `generation_options` in the preview/report. There
is no fixed count ceiling. The output-token budget is `512 × N`, checked for
arithmetic overflow before embedding/dictionary/credential work. Provider limits
and the 64 KiB response cap still apply; large requests can fail.
There is no automatic splitting, clamping or retry.

Generation uses the preview flags plus `--allow-model-call`. It needs
`OPENAI_API_KEY` in the process environment and a
[dictionary](DICTIONARY.md#install-and-use-offline); `.env` is not loaded.
It requests all candidates in one provider attempt, with no retry,
repair, acceptance or automatic target selection.

Generation can prepare missing vectors when explicitly given embedding backend
flags. Prefer the separate `prepare-retrieval` command when reviewing data
transmission. Local HTTP embeddings require `--embedding-provider local`, a
numeric loopback `--embedding-endpoint` ending in `/v1/`, and explicit model,
revision and dimensions. Hosted embeddings require `--embedding-provider openai`,
those identity fields, `OPENAI_API_KEY`, and **`--allow-embedding-call`** separately
from generation opt-in. Hosted embeddings transmit lexical documents for the
full eligible inventory and the brief, not source learner IDs/history. Cached
vectors avoid repeated document calls; a changed brief needs its own query vector.

Preview only reads files, validates, ranks cached vectors, prepares bytes and
renders. It constructs no dictionary, credential, HTTP client or runtime, and
never refreshes a missing/stale cache. It fails with `prepare-retrieval` guidance.

## Follow the execution

Start at the adjacent library functions `plan_generation` and `generate_story`
in [`crates/yomibu/src/story/mod.rs`](../crates/yomibu/src/story/mod.rs):

```text
plan_generation (offline)
  validate inputs/options/selection bounds
  selection::select_vocabulary (once)
  model_request::fit_selection_and_build_request (final selection + exact bytes)
  assessment::StoryAssessmentInputs::new (full original inventory)
  return immutable StoryGenerationPlan

caller initializes dictionary/client after preflight

generate_story (caller drives async I/O)
  client.generate_story_candidates (one attempt; unchanged bytes)
  assessment::assess_candidates (every text against full inventory)
  return owned StoryGenerationResult
```

Selection ranks cached cosine similarity, with explicit vocabulary targets first
in request order and ID-based ties for supports. Targets are never dropped to
fit the request budget; lowest-ranked supports may be removed during request
preparation. There is no second selection after generation. The final plan and
exact request bytes/hash are included in preview and generation reports.

## Assessment and output

The plan binds selection, exact request bytes/hash/options and full-inventory inputs.
The result owns original candidates, provenance, assessments and candidate errors;
it remains usable after inputs/resources are dropped. Every candidate is attempted.
Completed Fail/Inconclusive judgments, execution errors and NotRun stay distinct.
Vocabulary outside the selected plan but inside the full inventory is a plan
departure, not a vocabulary failure. Unresolved alternatives/competing identities
are inconclusive. An observed target survives unrelated uncertainty, with
`completeness: partial`; unsupported or target-specific evidence remains
unassessable. Grammar targets report occurrences only in bounded recognized
shapes. An object construction additionally requires the checker's unambiguous
direct-object lexical evidence; missing, false or competing evidence leaves its
grammar observations unassessable. Even a bounded object occurrence does not
resolve the object/predicate combination: the existing safeguard remains
Inconclusive. These observations do not establish contextual reading/sense, topic
adherence, naturalness or comprehension, and never trigger a retry.

JSON kinds are `story_generation_plan_preview` and `experimental_story_candidates`,
version 1. Prompt revision: `story-inventory-v1`. Text/JSON escape terminal
controls while retaining original UTF-8 candidate spans and decoded request bytes.
A candidate execution error exits 1 **after** writing every result. Completed
Fail/Inconclusive results exit 0; provider/preflight errors exit 1 without a
candidate report. CLI parsing errors exit 2.

## Limits and provider contract

Limits: manual input 4 MiB; WaniKani cache 64 MiB; request file 64 KiB; embedding
cache 128 MiB; 10,000 vocabulary entries; 128 grammar descriptions; 512 bindings;
16 vocabulary and 16 grammar targets; selection 1–16 entries; brief 2,048 bytes.
Inventory IDs are at most 128 bytes; written forms 256; up to 32 readings of
256 bytes and 32 meanings of 1,024 bytes; grammar descriptions 1,024 bytes.
The joined embedding document for each word (including labels/separators) must
fit 32 KiB; the entire set is checked before provider work. HTTP embedding batches
also split at the 512 KiB encoded request limit, accounting for JSON escaping.
The final provider request remains at most 16,384 bytes. Each candidate remains
nonblank and at most 100 Unicode scalars. The output-token budget scales with
count. OpenAI Responses uses `gpt-6-luna`, Standard/default tier, reasoning
`none`, no tools/streaming/background work, `store: false`, truncation disabled
and explicit prompt caching without breakpoints. Redirects, system/environment
proxies and protocol retries are disabled. Connect timeout is 5 seconds; the
30-second request deadline includes body reads. Responses are capped at 65,536
bytes, checking declared size and every chunk. Errors expose no keys or provider
response bodies. Completed envelopes require one completed assistant text payload;
documented reasoning items are ignored.
Refusal, incompleteness/errors, unknown outputs/tool calls, missing required
fields, invalid JSON/UTF-8, duplicate/extra payload fields and wrong types reject
the whole response without salvaging candidates. Optional usage/tier/request ID
may be absent; identity and usage are provider claims, not billing verification.
The parser requires exactly the requested count; otherwise it fails without a report.

## Evidence and history

See [analysis limits](ANALYSIS.md#input-and-supported-checks) and the
[history index](history/README.md) for frozen research, retired commands and code pins.
