# Shared story generation

The current experimental path accepts either manual material or an explicit
WaniKani cache. Source conversion ends at `LearnerInventory`; both sources use
the same request, selection, generation and assessment code. This replaces the
G2 CLI, without aliases. G0 preview, preparation, sync, status, analyze and the
G1 `generate-candidates` experiment remain unchanged.

"Story" includes short scenes and dialogues, including a single sentence. The
rename adds no plot/character requirements and does not expand the current
single-sentence generation limits.

## Inputs and meaning

The public module is `yomibu::story`. The previous `reading` module and
`preview-reading`/`generate-reading` commands have no compatibility aliases.

- **Learner inventory** is the complete material available for this attempt.
  WaniKani eligibility uses the existing explicit `lesson-started` or
  `recorded-pass` policy. This is revisable eligibility, not a mastery claim.
- **Story request** supplies a free-form `brief` and lists of vocabulary and
  grammar **targets**. A target can be any number of existing entries within
  the limits; it never grants new material. Empty target lists are valid.
- **Story generation options** control execution independently of the desired text.
  `candidate_count` defaults to 2 and maps to CLI `--candidates N`. Repair rounds
  would belong here when repair exists; no unused repair configuration is present.
- **Story generation plan** is the selected vocabulary sent with the brief, targets,
  grammar descriptions and executable bindings. This small prompt subset does
  not replace the complete inventory used for local assessment.
- **AI model request** (`AiModelRequest`) is the finalized serialized payload and
  its hash, with the encoded execution options. It contains no assessment inputs.
- **Story assessment inputs** (`StoryAssessmentInputs`) keep the full inventory,
  targets, final plan and structural-check data separate from the outgoing payload.

There is no extra user-facing `constraints` file or `context` file. The complete
inventory supplies the vocabulary/grammar constraints; the plan records what
this particular attempt sends. No separate manual and WaniKani generation paths
exist. No whole account, progress history or unselected vocabulary goes to the
generation provider.

[Manual inventory](../tests/fixtures/story/inventory.json):

```json
{
  "version": 1,
  "vocabulary": [
    {"id": "cat", "written_form": "猫", "readings": ["ねこ"],
     "meanings": ["cat"], "direct_object": null}
  ],
  "grammar_declarations": [{"id": "polite", "description": "polite nonpast"}],
  "grammar_bindings": [{"declaration_id": "polite", "rule": "PoliteNonPast"}]
}
```

[Story request](../tests/fixtures/story/request.json):

```json
{
  "version": 1,
  "brief": "A cat sleeping",
  "targets": {"vocabulary": ["sleep", "cat"], "grammar": ["polite"]}
}
```

Use both linked fixtures together; the abbreviated inventory above has no
`sleep` entry. IDs are stable strings. WaniKani entries use `wanikani:<subject-id>`.
Manual input can supplement a WaniKani projection, particularly its grammar;
duplicate IDs are rejected rather than silently overriding entries. A grammar
description records familiarity. A binding names one of the existing six
recognizers; descriptions never create executable rules.

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
is no arbitrary 4/8 upper cap. The output-token budget is `512 × N`, checked for
arithmetic overflow before embedding/dictionary/credential work. Provider limits
and the existing 64 KiB response cap still apply: a large count may be rejected
or produce an incomplete/oversized response. There is no automatic splitting,
clamping or retry, and no guarantee that every positive count is serviceable.

Generation uses the same flags as preview, plus `--allow-model-call` and either
`--dictionary PATH` or `--dictionary-dir PATH`. It needs `OPENAI_API_KEY` in the
process environment. As before, `.env` is not loaded. Each invocation requests
the requested number of experimental sentence candidates in one provider attempt, with no retry,
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

Start at `src/story_command.rs::run_generation`:

```text
load_generation_inputs (including execution-option validation)
  -> load_or_prepare_embeddings
  -> select_vocabulary
  -> build_ai_model_request (final plan + AiModelRequest)
  -> StoryAssessmentInputs::new (full original inventory)
  -> dictionary.load
  -> request_candidates_from_openai (credential, client and Tokio I/O event loop)
  -> assess_candidates
  -> write_generation
  -> require_completed_assessments
```

Selection ranks cached cosine similarity, with explicit vocabulary targets first
in request order and ID-based ties for supports. Targets are never dropped to
fit the request budget; lowest-ranked supports may be removed during request
preparation. There is no second selection after generation. The final plan and
exact request bytes/hash are included in preview and generation reports.

Direct library composition (caller owns I/O and the runtime):

```rust,ignore
let options = StoryGenerationOptions { candidate_count: 3 };
options.validate()?;
request.validate(&inventory)?;
let plan = select_vocabulary(&inventory, &request, &cache, &cache.model, 12)?;
let (plan, ai_request) = build_ai_model_request(&inventory, &request, plan, options)?;
let inputs = StoryAssessmentInputs::new(&inventory, &request, &plan)?;
// Initialize explicitly supplied execution resources only after preflight.
let analyzer = SudachiAnalyzer::load(dictionary_path)?;
let client = Client::new(api_key)?;
let generated = client.generate_story_candidates(&ai_request).await?;
let assessments = assess_candidates(&generated, &inputs, &analyzer);
```

`AiModelRequest` owns only the finalized payload, hash and encoded execution options.
`StoryCandidates` owns returned texts and metadata without borrowing a request.
`StoryAssessmentInputs` separately borrows the original full inventory/request and
final plan, and owns the structural-check projection. Pass these same inputs
explicitly to assessment; the selected subset never replaces the full inventory.
All original texts survive candidate-specific errors, and each assessment is attempted.
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
version 1. New prompt revision: `story-inventory-v1`. Text/JSON escape terminal
controls while retaining original UTF-8 candidate spans and decoded request bytes.
A candidate execution error exits 1 **after** writing every result. Completed
Fail/Inconclusive results exit 0; provider/preflight errors exit 1 without a
candidate report. CLI parsing errors exit 2.

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
count; other provider settings, deadlines,
response limits and one-attempt transport remain unchanged. The parser requires
exactly the requested count, otherwise the response fails without a candidate
report. G1 and historical G2 keep their fixed two-candidate contracts.

## Migration and boundaries

`generate-focused`, `context-preview`, `--permissions` and numeric `--focus-entry`
are removed from the current CLI. Use the commands above and stable target IDs;
there are no compatibility aliases or serialized-name disguises. Current JSON
and request hashes intentionally change. G1/analyze formats are unchanged.
Historical G2 low-level library APIs, prompt fixtures and comparison artifacts
remain for reproducibility, explicitly separate from current usage.

A1's historical **no-go** and object-combination safeguard remain in force.
This delivers experimental candidates, not stories or validated exercises.
Sync/status/preparation/analysis behavior and frozen A1 evidence are unchanged.

## Tokio naming

Tokio's `Runtime` is an event loop/executor that drives async network I/O and
timers. This CLI uses a single-threaded runtime. `block_on(future)` is a Tokio
method, not a Rust standard-library method: it drives that async operation to
completion while the synchronous CLI waits. Calling an async Rust function alone
creates a future; something must drive it. Library callers use `.await` inside
their own async environment. The CLI names its local value `io_runtime` and keeps
construction/`block_on` inside the concrete provider helper, outside the high-level
sequence. This needs no generic executor abstraction.
