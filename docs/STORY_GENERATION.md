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

The complete TOML references are [application settings](../config/config.toml),
[pipeline defaults](../config/default-pipeline.toml), and
[story defaults](../config/default-story.toml). Their active values equal omission.
They are not installed automatically. Flat files and old component CLI spellings are
rejected; old ENV names are ignored. There are no aliases or migrations.

Storage uses `--data-dir` → `YOMIBU_DATA_DIR` → `$HOME/.yomibu`.
`--config` → `YOMIBU_CONFIG` selects `config.toml`; otherwise use the data directory.
The two defaults files are siblings of that selected file, even if it is absent.
Missing implicit files are empty; a missing explicit config or another read error
fails. Each document is bounded to 64 KiB of UTF-8. Application paths are relative
to the config directory; CLI/ENV paths use the working directory.

Application settings own resources, sync policy and hosted-call authorization.
Pipeline settings own component choices (including source and storage), component
options and selection ordering. Story settings own topic,
targets, seed, limit, format and candidate count.

Ordinary precedence is CLI → ENV → saved defaults → built-ins, independently per
field. Lists replace whole lists. Empty target lists clear targets; empty selection
lists are invalid. Optional values accept exactly `{ clear = true }` in TOML or
`Patch::Clear` in typed invocations. Clearing restores absence or the computed
resource default, including implicit-path behavior. It does not clear related
settings. Required fields use values, optional features use booleans; there is no
ENV clear sentinel. Empty strings do not mean clear.

The generation component's optional model resolves independently, then falls back
to `pipeline.model` (`--model`). Thus a saved generation model beats a CLI shared
model. `--openai-model` and `YOMIBU_OPENAI_MODEL` come from the component declaration.
HTTP encoder options use `--http-embeddings-{model,revision,dimensions,endpoint}`
and matching uppercase `YOMIBU_HTTP_EMBEDDINGS_…` names. Names never use Rust types.
Existing story, resource and policy flags remain explicit. Repeat `--enable` /
`--disable` for sync, embeddings or assessment; ENV controls are comma-separated.
A same-source enable/disable conflict fails. Mandatory core validation cannot be
disabled.

Credentials belong to components. Names generate `--<component>-<credential>` and
`YOMIBU_<COMPONENT>_<CREDENTIAL>` inputs. The components are `openai`, `wanikani`,
and `http-embeddings`, each declaring `api-key`. Hosted embeddings need their own
key; generation's key is never reused automatically.

Resolution is CLI → ENV → supplied value (Keychain on macOS). A winning blank key
is missing and never falls through. Invalid encoding fails credential-dependent
work; optional story embeddings warn and use the configured base selection.
Values never belong in TOML, reports or Debug output. CLI diagnostics redact keys.
Components receive credentials explicitly.

On macOS, `yomibu auth` prompts for missing credentials of enabled integrations.
`yomibu auth http-embeddings` prompts for that component's missing credentials,
even before enabling it. `yomibu auth http-embeddings.api-key` sets or replaces
that key. Enter skips any prompt. Generation is required for stories; sync and
hosted embeddings are optional during setup.
Keychain uses service `yomibu:<component>` and account `<credential>`.
Reads are lazy. `--no-keychain` disables lookup. Normal commands never ask for keys.
Other platforms use CLI/ENV inputs. No plaintext fallback.

WaniKani requires read access, OpenAI generation
requires response creation and model access, and hosted embeddings require
embedding access.

All commands read application and relevant pipeline settings. Story, preview and
retrieval also load story defaults. Sync uses the selected source/store regardless
of automatic-sync policy; status uses only the store. Analyze uses the selected
analyzer/checks; dictionary commands use its dictionary resources. They ignore
story and text-generation settings. Unknown paths and malformed loaded TOML fail
without reflecting contents. Known unused fields are pruned before type checking;
consumed lower-precedence inputs must still have valid types.

`LocalApp::for_invocation(Invocation { pipeline, story }, operation)` resolves an
independent snapshot through the same typed application logic as file/CLI loading.
It shares immutable resource settings and credentials. Request overrides cannot
choose resource paths or authorization. No shared current pipeline is mutated.

Default selection is `lexical-topic` then `seeded-order` (`builtin-v2`).
`seeded-order` alone uses `seeded-only-v1`. Embedding selection is `embedding-rank`
(`inventory-similarity-v1`), optionally followed by `seeded-order`
(`inventory-similarity-seeded-v1`). Other sequences, duplicates and empty lists
fail application validation. Library compositions remain unrestricted; core's
target-first finalization is mandatory and the full allowed inventory stays
separate from prompt selection.

Embeddings require `pipeline.selection.embeddings = true` and a topic. The
embedding sequence never authorizes hosted calls. Complete caches are reused
before credential resolution or provider construction. Missing/stale evidence may
be prepared only under application policy; optional failure warns and falls back
to the configured base sequence. Preview only reads caches. Explicit retrieval
prepares evidence regardless of optional enablement. See [retrieval](RETRIEVAL.md).
Sync and optional assessment default on. Disabled work does no resource I/O.
Local story assessment initializes after generation, keeps dictionary lifetimes, warns
on optional failures and preserves generated text and NotRun behavior.
`LocalApp::story_with_inputs` safely borrows an initialized Sudachi analyzer.
`None` skips analysis without consulting dictionary paths; disabled assessment
ignores the analyzer. Initialization retains the [dictionary lifetime contract](DICTIONARY.md#file-stability-contract).

## Topic and vocabulary

`--topic` is optional and conflicts with `--request PATH`. Typed topic overrides,
including clear, have the same conflict. Advanced request JSON
uses `topic` plus target IDs. Omitted topic inherits saved defaults; `null` clears
it. Both target arrays remain required and replace saved lists. The request file
is invocation input, not a persisted defaults reference. Without a topic, the AI creates a coherent scene
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
