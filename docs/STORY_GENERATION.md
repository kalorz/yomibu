# Shared story generation

`story` creates a passage of 3–5 short sentences with one generation attempt.
Use `--format sentence` for one sentence. No topic, dictionary, or embeddings are
required. These readings remain experimental; verification, acceptance and repair
have not run. See the [module map](../ARCHITECTURE.md#current-module-map).

## Inputs and meaning

Use the complete [inventory](../tests/fixtures/story/inventory.json) and
[request](../tests/fixtures/story/request.json) examples together.

- The inventory contains all allowed material. WaniKani eligibility uses explicit
  `lesson-started` or `recorded-pass` policy; eligibility does not prove mastery.
- The request contains an optional `topic` and vocabulary/grammar target IDs.
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

## Configuration and optional work

Supply `YOMIBU_WANIKANI_API_KEY` and `YOMIBU_OPENAI_API_KEY`, or the corresponding
`--wanikani-api-key` and `--openai-api-key` flags. Flags win. Credentials are never
saved or included in reports. WaniKani needs [read access without write permissions](https://docs.api.wanikani.com/20170710/#authentication).
OpenAI needs [response creation and selected-model access](https://developers.openai.com/api/docs/guides/terraform/service-accounts#assign-least-privilege-permissions).
A usable cached/manual inventory removes the WaniKani-key requirement.

Storage uses `--data-dir`, `YOMIBU_DATA_DIR`, or `$HOME/.yomibu`. Optional
`config.toml` lives there; `--config` or `YOMIBU_CONFIG` selects another file.
Non-secret settings resolve flags → supported `YOMIBU_` bindings → file → defaults.
Environment names use the uppercase setting name. File paths are relative to the
configuration file. Commands validate only settings they use. Invalid TOML and
unknown settings fail without reflecting their contents.

```toml
model = "gpt-6-luna"
# generation_model overrides the shared text fallback after source resolution.
disable = ["assessment"]
# inventory = "inventory.json"
```

`--model` supplies a text fallback; `--generation-model` overrides it.
Embedding provider/model/revision/dimensions are separate. Configuring them does
not enable embeddings. Partial settings require a provider; they never override
an unrelated cached model. Repeat `--enable MODULE` / `--disable MODULE` for `sync`,
`embeddings`, and `assessment`. Saved `enable`/`disable` lists work the same way;
environment lists use commas. CLI controls win; conflicting controls fail.
Disabled modules perform no initialization or I/O. Explicit resource commands
still require their resources.

Sync and available local assessment default to enabled. Embeddings require
explicit enablement. Hosted embeddings additionally require
`--allow-embedding-call`; see [retrieval](RETRIEVAL.md). No dictionary is downloaded.
Optional failures warn and preserve generated text, using built-in selection when
retrieval fails. Preview checks the same embedding settings and reports fallback
warnings. It remains offline and never refreshes resources.

## Topic and vocabulary

`--topic` is optional and conflicts with `--request PATH`. Advanced request JSON
uses `topic` plus target IDs. Without a topic, the AI creates a coherent scene
around locally sampled vocabulary. `--seed` makes selection repeatable.
With a topic, built-in selection matches lexical terms and Japanese written-form
substrings. Readings still use exact terms. Enabled embeddings can enhance selection.
No topic means no query retrieval. Explicit targets
always come first. Preparation may trim supports to fit the request limit;
it never drops targets. Simple grammar guidance does not infer grammar knowledge.

## Cache

A validated usable WaniKani cache from the last hour is reused without requests.
Missing/stale data triggers a complete refresh, with freshness rechecked under
the writer lock. Temporary transport/server/rate-limit failures or writer
contention may reuse a validated usable cache with a warning. An old cache without
a key also warns. Disabled sync requires usable cached/manual knowledge.
Authentication, account mismatch, corruption, invalid data, expired content access,
and persistence failures remain errors. Preview and retrieval also reject expired
content. `cache_max_age_seconds` can change the product default. `status` stays offline; `sync` always refreshes.
WaniKani [recommends caching](https://docs.api.wanikani.com/20170710/#caching);
conditional/incremental requests remain deferred.

## Assessment and output

The workflow binds selection, exact request bytes/hash/options and full-inventory inputs.
The result owns original candidates, provenance, assessments and candidate errors;
it remains usable after inputs/resources are dropped. Available assessment
checks every sentence.
Completed Fail/Inconclusive judgments, execution errors and NotRun stay distinct.
Vocabulary outside the selected plan but inside the full inventory is a plan
departure, not a vocabulary failure. Unresolved alternatives/competing identities
are inconclusive. An observed target survives unrelated uncertainty, with
`completeness: partial`; unsupported or target-specific evidence remains
unassessable. Grammar targets report occurrences only in bounded recognized
shapes. Object を requires unambiguous direct-object lexical evidence;
missing, false or competing evidence leaves that rule unassessable.
Topic and polite-form observations retain their independent evidence.
Even a bounded object occurrence does not
resolve the object/predicate combination: the existing safeguard remains
Inconclusive. These observations do not establish contextual reading/sense, topic
adherence, naturalness or comprehension, and never trigger a retry.

Normal stdout contains the passage; warnings use stderr. `--verbose` adds module
states, progress and monotonic step timings to stdout. `--json --verbose` sends
progress to stderr and writes one JSON report, including states and timings.
An absent default dictionary skips assessment. Explicit unusable dictionary
selections warn and retain generated text.
Missing/disabled assessment returns `NotRun`; missing grammar knowledge leaves
grammar-dependent checks unassessed. Optional assessment errors exit zero with
warnings. Provider/preflight failures exit 1; parsing failures exit 2.
Reports retain assembled text and sentence byte ranges. Terminal escaping preserves
decoded JSON strings and original spans. JSON kinds are
`story_generation_plan_preview` and `experimental_story`; prompt revision is
`story-inventory-v3`.

Evaluation JSON includes `basis: full_learner_inventory`. Target `uncertainties`
include `span`, `scope`, `reason` and `inventory_entries`. Reasons use snake-case
`code` values and `detail` for nested evidence.

## Limits and provider contract

Limits: manual input 4 MiB; WaniKani cache 64 MiB; request file 64 KiB; embedding
cache 128 MiB; 10,000 vocabulary entries; 128 grammar descriptions; 512 bindings;
16 vocabulary and 16 grammar targets; selection 1–16 entries; topic 2,048 bytes.
Inventory IDs are at most 128 bytes; written forms 256; up to 32 readings of
256 bytes and 32 meanings of 1,024 bytes; grammar descriptions 1,024 bytes.
The joined embedding document for each word (including labels/separators) must
fit 32 KiB; the entire set is checked before provider work. HTTP embedding batches
also split at the 512 KiB encoded request limit, accounting for JSON escaping.
Story preparation caps the final encoded OpenAI request at 16,384 bytes.
Each sentence remains nonblank and at most 100 Unicode scalars. Strict sentence
arrays contain one sentence or 3–5 per passage. The output-token budget is
`512 × maximum sentences × candidates`, checked for overflow. `--candidates`
defaults to 1; no automatic splitting, clamping or retry occurs.
OpenAI Responses uses the resolved model (`gpt-6-luna` by default), Standard/default tier, reasoning
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
