# Shared story generation

`story` creates a passage of 3–5 short sentences with one generation attempt.
Use `--format sentence` for one sentence. No topic is required.
These readings remain experimental; verification, acceptance and repair
have not run. These are current behavior contracts;
the [architecture guide](../AGENTS.md#architecture) describes the target organization.

## Inputs and meaning

Local generation derives eligibility from supplied normalized material and progress.
`run_story` accepts a caller-prepared allowed inventory and typed request.
Vocabulary requires a recorded lesson completion; hidden/unavailable entries are excluded.
The request holds an optional topic and vocabulary/grammar target IDs.
Targets must exist in the inventory. Selection is only the prompt subset.

IDs are stable strings. WaniKani entries use `wanikani:<subject-id>`.
Readings and meanings remain separate source alternatives, never a Cartesian
product of asserted word uses. WaniKani accepted-answer alternatives are kept;
kana-only records keep missing readings rather than inventing them. Raw readings
are preserved.
`direct_object: null` means unknown; WaniKani glosses/POS do not infer transitivity.
A single reading and meaning still does not establish contextual correctness.
The original source cache is unchanged and remains the evidence record; the
inventory retains source subject IDs and exclusions.

## Configuration and optional work

The TOML references are [configuration location](../config/config.toml),
[pipeline defaults](../config/default-pipeline.toml), and
[story defaults](../config/default-story.toml). Their active values equal omission.
They are not installed automatically. Flat files and old component CLI spellings are
rejected; old ENV names are ignored. There are no aliases or migrations.

Storage uses `--data-dir` → `YOMIBU_DATA_DIR` → `$HOME/.yomibu`.
`--config` → `YOMIBU_CONFIG` selects `config.toml`; otherwise use the data directory.
Keep `config.toml` empty. The two defaults files are its siblings, even if it is absent.
Missing implicit files are empty; a missing explicit config or another read error
fails. Each document is bounded to 64 KiB of UTF-8.

Pipeline settings own component choices (including source and storage), component
options and selection ordering. Story settings own topic,
vocabulary targets, seed, limit, format and candidate count.
Typed option declarations supply CLI/environment names, file validation, help and
auth guidance. Components receive scoped options and explicit credentials.

Ordinary precedence is CLI → ENV → saved defaults → built-ins, independently per
field. Lists replace whole lists. Empty target lists clear targets; empty selection
lists are invalid. Optional values accept exactly `{ clear = true }` in TOML or
`Patch::Clear` in typed invocations. Clearing restores absence or the default.
It does not clear related settings. There is no ENV clear sentinel.
Empty strings do not mean clear.

`--openai-model`, `YOMIBU_OPENAI_MODEL`, and `pipeline.options.openai.model`
select the model, with `gpt-6-luna` as the built-in default.

Credentials belong to components. Names generate `--<component>-<credential>` and
`YOMIBU_<COMPONENT>_<CREDENTIAL>` inputs. The components are `openai` and `wanikani`,
each declaring `api-key`.

Resolution is CLI → ENV → supplied value (Keychain on macOS). A winning blank key
is missing and never falls through. Invalid encoding fails credential-dependent
work.
Values never belong in TOML, reports or Debug output. CLI diagnostics redact keys.
Components receive credentials explicitly.

On macOS, `yomibu auth` prompts for missing OpenAI and WaniKani credentials.
`yomibu auth openai` prompts for its missing key; `yomibu auth openai.api-key`
sets or replaces it. Enter skips any prompt.
Keychain uses service `yomibu:<component>` and account `<credential>`.
Reads are lazy. `--no-keychain` disables lookup. Normal commands never ask for keys.
Other platforms use CLI/ENV inputs. No plaintext fallback.

WaniKani requires read access, OpenAI generation
requires response creation and model access.

All commands read relevant pipeline settings. Story also loads story defaults.
Unknown paths and malformed loaded TOML fail
without reflecting contents. Known unused fields are pruned before type checking;
consumed lower-precedence inputs must still have valid types.

`LocalApp::for_invocation(Invocation { pipeline, story }, operation)` resolves an
independent snapshot through the same typed application logic as file/CLI loading.
It preserves resource settings and shares credentials. Request overrides cannot
choose resource paths or authorization. No shared current pipeline is mutated.

Default selection is `lexical-topic` then `seeded-order` (`builtin-v2`).
`seeded-order` alone uses `seeded-only-v1`. Other sequences, duplicates and empty lists
fail application validation. Library compositions remain unrestricted; core's
target-first finalization is mandatory and the full allowed inventory stays
separate from prompt selection.
Core checks each step against the full allowed inventory; external steps may
restore allowed entries. Built-in selection steps retain filtered subsets.

## Library boundaries

`application::run_story` takes resolved configuration, a typed request and inventory,
and a borrowed generation client. It neither reads storage nor accesses WaniKani.

`prepare_story` binds the finalized selection to a `PreparedStory<R>`.
`PreparedStory::generate` requires a generator with the same associated request type
and checks candidate count and format. OpenAI owns exact request bytes and model
validation; prompt preparation owns content, byte limits and support trimming.
Checked candidate construction preserves original text and complete UTF-8 sentence spans.

## Topic and vocabulary

`--topic` is optional. Typed requests and saved defaults can specify practice targets.
Without a topic, the AI creates a coherent scene
around locally sampled vocabulary. `--seed` makes selection repeatable.
With a topic, built-in selection matches lexical terms and Japanese written-form
substrings. Readings still use exact terms. Explicit targets
always come first. Preparation may trim supports to fit the request limit;
it never drops targets. Simple grammar guidance does not infer grammar knowledge.

## Cache

Run `yomibu sync` before generation. Local storage uses `<data-dir>/wanikani.json`.
The file adapter maps saved WaniKani data into normalized material, progress and
access expiry before story generation.
Story reads one snapshot without refreshing, writing, or looking up source credentials.
Cache age alone does not warn or block generation. Missing data, corruption,
invalid data and expired content access remain errors.

## Output

Normal stdout contains generated passages; errors use stderr. `--verbose` adds progress,
monotonic step timings and generation details to stdout.
Provider/preflight failures exit 1; parsing failures exit 2.
Reports retain assembled text, provenance and sentence byte ranges. Terminal
escaping keeps Japanese readable. Prompt revision is `story-inventory-v3`.

## Limits and provider contract

Limits: WaniKani cache 64 MiB;
10,000 vocabulary entries; 128 grammar descriptions; 512 bindings;
16 vocabulary and 16 grammar targets; selection 1–16 entries; topic 2,048 bytes.
Inventory IDs are at most 128 bytes; written forms 256; up to 32 readings of
256 bytes and 32 meanings of 1,024 bytes; grammar descriptions 1,024 bytes.
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

See the [history index](history/README.md) for frozen research, retired commands and code pins.
