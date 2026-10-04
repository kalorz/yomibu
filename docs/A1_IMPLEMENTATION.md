# A1 implementation and evidence status

Updated 2026-10-04. Implementation was explicitly approved after the protocol-only stage.
The synchronous prototype, real analyzer adapter, and evaluation example are
implemented. **A1 investigation complete: no-go.** The held-out targets passed;
three unsupported visible challenge Pass results failed the separate safeguard.
The [evaluation status](A1_EVALUATION_STATUS.md) records the frozen implementation,
single held-out run, private scoring receipt and preserved span discrepancy.
Contract tests and smoke results remain engineering evidence; they do not
establish independent Japanese ground truth.

The separate [object-combination follow-up](A1_FOLLOWUP.md) starts at local
checkpoint `f1237d4`. Current code is a later revision; the completed A1 results
above still describe the frozen implementation. No held-out rerun or rescore is
part of the follow-up.

## Run the offline example

From the repository root, with Python 3.8+ for the one-time setup:

```sh
python3 scripts/setup_a1_dictionary.py
cargo run --locked --example a1 -- target/a1/system_core.dic tests/fixtures/a1/smoke.json
```

Setup explicitly downloads the pinned public dictionary archive (about 72 MB),
checks its bytes and SHA-256, extracts only the required dictionary and publisher
notices, checks the dictionary, and replaces files only after verification.
The archive can instead be supplied with `--archive /path/to/pinned.zip`.
Existing usable dictionary bytes remain until the verified replacement is ready.
Files stay under ignored `target/a1/`; no dictionary is committed. Deleting target
requires setup again. Neither the library nor example downloads anything.

The example accepts exactly a dictionary path and synthetic JSON path. It has no
learner-store, environment credential, sync, HTTP, or model composition. The
ordinary Yomibu CLI is unchanged. The three smoke cases exercise completed Pass,
Fail, and Inconclusive reports. They are not reference-acceptance cases.

JSON output retains the exact inputs/bindings, C/A tokens, original byte spans,
provenance, five required checks, findings, unassessed aspects, and the bounded
outcome. `{"Completed":"Pass"}` is distinct from `"NotRun"`. An execution error
sets all checks/outcome for that case to NotRun and records its error chain;
other cases still run. Missing/mismatched dictionaries produce explicit execution
errors, never a fallback analyzer. Invalid packet envelopes fail before analysis.
Exit status is nonzero for input/execution errors. Completed Fail/Inconclusive
judgments have exit status zero: it means the run completed, not acceptance.
Reference evidence and accuracy are deliberately not inferred by this executable.

Keep detailed evaluation output outside Git and external services. For a scored
run, the custodian records the code commit (and any diff), Rust/platform version,
packet, binding, reference, prompt, and output hashes. Save outputs before joining
private labels. The example never reads a reference key.

## Verified dependency and configuration pins

| Item | Pin |
| --- | --- |
| Sudachi.rs | v0.6.11, Git `90fd6068c80c2fc3b63e0dbab0e341475bad4d8f` |
| Dictionary | SudachiDict Core 20260723 **V0** |
| Publisher ZIP | 72,276,502 bytes; SHA-256 `b6e835f63440f97474c2da45d80950f73746e632e40bbfc168b4041729135e1f` |
| Extracted dictionary | 217,466,039 bytes; SHA-256 `53fa281d11eef3769712fe1c3c892117338f9892bee6daf4dad51daa5281bb6f` |
| Built-in configuration | `src/adapters/sudachi.json`; SHA-256 `45cde6f1eba960c32475e267dfa422e51b1f215e3fa142162079d711eff77e4c` |

The hashes were calculated from the exact official HTTPS download on 2026-10-03;
they are reproducibility pins, not publisher signatures or independently supplied
checksums. Compatibility was exercised by loading this pair and running real
C/A adapter tests. Upstream documents the V0/V1 break for 0.6/0.7; there is no
upgrade to a floating 0.7 release.

Sources: [v0.6.11 release](https://github.com/WorksApplications/sudachi.rs/releases/tag/v0.6.11),
[pinned implementation](https://github.com/WorksApplications/sudachi.rs/tree/90fd6068c80c2fc3b63e0dbab0e341475bad4d8f),
[SudachiDict compatibility/licensing](https://github.com/WorksApplications/SudachiDict),
[exact publisher archive](https://sudachi.s3.ap-northeast-1.amazonaws.com/sudachidict/sudachi-dictionary-20260723-core.zip).
Sudachi.rs is Apache-2.0. The dictionary archive's `LEGAL` and `LICENSE-2.0.txt`
are retained by setup, including embedded UniDic and NEologd notices. Do not
replace these notices with the synthetic fixture reuse terms.

The adapter uses embedded character definitions, no input normalization, no path
rewrites, and only the built-in SimpleOovPlugin with the committed parameters.
It does not search the working directory for configuration, rewrite/OOV files,
user dictionaries, or dynamic plugins. A child-process test supplies poisoned
ambient files and verifies unchanged operation/configuration hash. This minimal
configuration is a deliberate evaluation pin; it is not Sudachi's default recipe
or a claim of optimal segmentation. Out-of-dictionary results remain unresolved.

The adapter owns the verified dictionary bytes (roughly 217 MB plus analyzer
structures). This avoids a file-change race between checksum verification and
memory-mapped access. Share/reuse one analyzer for multiple sentences. Startup
hashing is visible in debug test runtimes. No application unsafe code was added;
upstream dictionary internals are trusted only after exact-byte verification.

## Library behavior and limits

`analysis::Sentence` borrows unchanged text and validates nonblank input and a
100-Unicode-scalar limit. `adapters::sudachi::SudachiAnalyzer` returns C whole units
and A components with original UTF-8 spans, dictionary-form/reading/POS hypotheses,
OOV flags, and provenance. The selected tokenizer path is evidence, not truth.

`evaluation::evaluate` takes that analysis, unchanged `GrammarDeclarations`, and
explicit `EvaluationBindings`. Whole-word spelling/reading/sense tuples remain
associated. Identical duplicates do not imply ambiguity; competing supplied
identities, reading mismatches, and OOV identities remain Inconclusive. Recognized
unlisted words Fail. Components never substitute for a C-mode whole permission.
For supported regular inflections, the base spelling/reading is checked against
the observed stem using the dictionary's class; no -iru/-eru class guessing.
A sense label is retained, not semantically validated against context.

The [common binding sheet](A1_BLIND_PACKET_HEADER.md) specifies the exact two
single-clause patterns, seven rule bindings, punctuation limits, and five checks.
`direct_object` is an explicit claim tied to one vocabulary tuple, requiring
source audit; it is not inferred from を. False means no positive binding, not
proof of intransitivity. Descriptions never become grammar permissions by parsing.

Malformed spans, missing declaration IDs, and blank vocabulary identities are
typed execution/input errors. Unsupported constructions make grammar applicability
Inconclusive. Supported permission failures remain Fail with a reason/span even
when other checks are unresolved. All findings are retained.

**Current coverage restriction:** the separately recorded follow-up keeps
object/predicate combinations unresolved even with transitive-use evidence.
Particles and Scope are Inconclusive when permissions are present; existing
permission failures remain Fail with their spans and the combination limitation
alongside them. This includes ordinary object sentences. The finding covers the
object through the predicate, without a preceding topic or final 。.

The frozen A1 implementation did pass three literal-looking multiword challenges;
that historical safeguard failure is unchanged. Current code adds no MWE detector
or contextual validator. Other semantic/discourse/register alternatives can still
share a supported surface shape. Every report retains multiword expressions,
contextual reading/sense, and naturalness as unassessed. Structural checks do not
establish an accepted exercise or justify a new go decision.

## Reference history and current evaluation

The [active review draft, revision 2](../tests/fixtures/a1/review-draft-v2.json) contains 24 development
and 12 challenge cases, original and explicitly reusable. It has now been frozen
and evaluated after the full reported reference gate was met. The implementer's provisional
initial judgments/alternatives are stored separately outside Git and preserved
unchanged alongside later per-claim adjudications. The author has seen contract
tests and overlapping text: the draft is exposed development material.
The [original draft](../tests/fixtures/a1/review-draft.json) remains byte-identical;
revision 2 replaces two entire development families after source clarification.

No held-out set was authored here, and its private case/review files have not
been opened. An earlier custody comment exposed linguistic hints; the 2026-10-04
v2 status receipt reports five complete affected families retired and replaced
(ten cases), with fourteen cases retained and all prior evidence preserved.
The mapping and family separation are custodian determinations, not independently
inspected here. The final receipt reports 24/24 current cases reviewed, reconciled
and frozen. A review covering only retired cases receives no credit.
Non-browsing reviewer citations remain model-only in the raw response; the
custodian reports corroboration of every decisive claim using actual opened
sources in a separate provisional adjudication. Agreement is not source evidence.
Record actual provider metadata in custody rather than inferring it from model
self-identification. The
[custodian handoff](A1_HOLDOUT_CUSTODIAN_PROMPT.md) keeps review/custody separate;
the full reference gate/freeze preceded benchmark-driven development.
The user operates external review manually. All six six-case replies and the
four replacement reviews have been
audited; the [sixth review packet](A1_SIXTH_REVIEW_PACKET.md), like the earlier
packets, remains an unchanged reviewed snapshot. All original cases are reviewed;
the [four-case blind packet](A1_FOUR_CASE_REVIEW_PACKET.md) also remains unchanged
after its completed audit.
Future new cases use the [review prompt](A1_EXTERNAL_REVIEW_PROMPT.md)
and binding sheet. Save raw answers, verify actual source passages, and retain
disagreement. Grammar source coverage
in the protocol does not verify each word, transitivity claim, or interpretation.

Current visible revision-2 coverage: **24/48 source-backed, provisionally
adjudicated core outcomes**, plus **12/12 challenge references**. The custodian
separately reports all 24 current held-out references reconciled, giving **48/48
combined reported core coverage**. Hidden source records were not inspected here.
The reported full reference gate is met and the exact reference artifacts frozen.
The [Scope clarification](A1_SCOPE_CLARIFICATION.md) applies existing v0.3 rules:
Scope assesses structural coverage/applicability and does not copy another check's
permission result. Preserve the reviewer's alternative and append any corrected
check cells before freeze; no new blind-review round solely for this clarification.
Historically, the original draft reached 22 supported core outcomes and two
disputed positives. Both members of each affected family were replaced, including
two supported negatives; none of their evidence is erased or counted twice.
All 36 original cases received blind reviews and separate source audits.
Actual TUFS grammar passages and named Shogakukan dictionary
entries were inspected; inaccessible Jisho citations and JMdict IDs remain
unverified. Overstated absence-of-alternative claims were narrowed, and uncertain
nominal/adjectival applicability changed two check judgments to Inconclusive.
The first six overall recommendations remain unchanged. Protocol v0.3 clarifies lexical
attestation versus contextual sense selection before any benchmark run; the v0.2
packet, raw response, initial judgments, and correction ledger remain preserved
outside Git. First-review provider/model/version/settings were not supplied and
stay unknown. The second review's provider/model names are self-reported by the
model, without confirmation from the service UI; that distinction is recorded.

The second audit retained all six overall recommendations and changed two
particle checks to Inconclusive. Sources establish the expression alternatives
and component morphology, but generic transitivity does not establish scoped
object applicability inside an unresolved expression. A dictionary expression
entry is not automatically a single whole word or a permission violation.
These corrections apply existing v0.3 rules; scope, thresholds, and code are
unchanged. Original reviews and reviewed packets remain preserved.

The third audit retained all six overall and thirty check recommendations.
Empty grammar bindings mean no permitted rules, while all five checks still run;
the reviewer's alternative interpretation is retained but not adopted under the
existing contract. New dictionary passages and the publisher's POS convention
were inspected; reused earlier source checks have explicit provenance. Claims
about exhaustive reading uniqueness or naturalness under every reading were
narrowed to attestation and bounded rule application. Unverified JMdict claims
remain unverified. Provider/model/settings were not supplied and stay unknown.

The fourth audit retained all six overall recommendations and corrected two
check judgments to Inconclusive. A cited dictionary also records adjectival use
with historical examples; its modern applicability remains unresolved. The
supported empty-vocabulary violation survives that grammar uncertainty. One
dictionary-base/original-span mismatch was corrected. Expression entries do not
establish single-word boundaries or automatic permission violations. All raw
reviews, alternatives, and corrections remain private; no runtime change followed.

The fifth audit withheld two proposed core positives because reading alternatives
were dismissed without sufficient evidence; one also retains noun/adjective
uncertainty. Four check judgments changed to Inconclusive. These are reference
limitations, not observed analyzer results, and no binary scoring key is granted
to those cases. A compound's initial provisional recommendation changed to Fail
after additional source text explicitly established the component's word-forming
role and named the compound. All initial proposals and reviewer alternatives
remain preserved. The reviewer's reported derived JMdict build and IDs remain
unverified; decisive claims use inspected publisher sources. No coordinator
dictionary download occurred.

The sixth audit retained all six overall recommendations and changed one
vocabulary check to Inconclusive, carrying forward the prior reading uncertainty.
A supported particle-permission violation survives because the retained readings
remain nouns in the recognized construction. Additional source text establishes
the full place-name and its abbreviation; component decomposition does not add
permission for the full identity. Unverified snippets/JMdict claims stay unverified.

Review of the exact binding sheet also exposed nine earlier absent-feature Pass
findings inside unresolved constructions. A separate append-only correction makes
those checks Inconclusive; original audits, raw replies, overall outcomes and gate
counts remain unchanged. Directly evidenced morphology is distinct from an empty
no-occurrence finding. This corrects application of the existing contract, without
altering the binding sheet or runtime. Reviewer self-identification as Claude from
Anthropic remains an attributed claim; service UI version/settings are unknown.

The single focused unblinded clarification has returned and was audited. Both
original positives remain unresolved; the sources did not supply the missing
reading/POS restrictions. Corrected an overstated source POS label and an absent
nominal-check rationale without promoting either reference. Reported model/version,
date and settings remain attributed metadata, not service UI verification.

Revision 2 uses the protocol's logged pre-freeze replacement route. It changes
four inputs together in their existing family lineages, preserving category and
12-positive/12-negative development proposals, all 12 challenges, and thresholds.
The four initial source-inspected proposals remain private and unchanged;
gate credit was withheld until blind review and reconciliation. Original cases,
reviews, exclusions and disagreements are retained. No analyzer execution guided
the choice; no behavior changed. Both original and revised families remain exposed
development material and must be excluded from holdout.

The four-case audit retained all four overall and twenty check recommendations.
The existing contract settles grammar-form permissions and the scoped topic
slot without resolving discourse roles. Reviewer alternatives remain recorded;
no scope change or new review is needed. Publisher noun/receiver definitions,
POS conventions and the topic/object frame were checked; earlier actual lexical
and morphology inspections have explicit provenance. The reported redistributed
JMdict file and snippet remain unverified and supply no decisive evidence.
Blanket uniqueness and component-boundary claims were narrowed. An aggregate
private ledger applies the earlier check corrections without rewriting history.
Reported model/version metadata remains attributed, not independently confirmed.

These counts measure reference source coverage, not model agreement or analyzer
accuracy. The [visible evaluation](A1_EVALUATION_STATUS.md) now matches all 24
development outcomes and, after a small reporting fix, all negative reason/spans.
Three unsupported challenge Pass results fail the challenge safeguard. The current
holdout v2 is reported frozen; its case files remain unseen here and its performance
targets are not evaluated. The implementation/configuration is frozen for the
single held-out run. Model agreement alone does not establish linguistic truth;
the recorded challenge failure prevents a bounded go regardless of that run.

## TDD and verification record

Every behavioral cycle began by running its new focused test and observing the
expected failure before the corresponding production change. Infrastructure,
fixture licensing, CI setup, and documentation did not require artificial REDs.

| Cycle | Observed RED | GREEN and refactor review |
| --- | --- | --- |
| Input boundary | Missing Sentence API | Borrowed newtype; nonblank/100-scalar checks; reviewed ownership and kept exact text; rerun passed |
| Real analyzer | Missing concrete adapter | Exact dictionary/configuration, C/A spans, typed failures; reviewed owned bytes and no ambient loading; rerun passed |
| Whole vocabulary | Missing evaluation API | Whole identity never inherits component permissions; reviewed minimal concrete types; rerun passed |
| Uncertainty/input validity | Ambiguous identities passed; malformed inputs were accepted | Preserve uncertainty and validate full byte/component coverage; reviewed duplicate handling and error boundaries; rerun passed |
| Nominal rule | Missing explicit bindings/check inventory | Nominal permission and declaration IDs; extracted vocabulary checking from orchestration; rerun passed |
| Godan polite forms | Missing four rule variants | Base/stem reading and exact suffixes; reviewed borrowing and finite rule matching; rerun passed |
| Ichidan/class distinction | Ichidan form was unresolved | Dictionary-class-directed stem logic, checked all regular endings; reviewed common stem helper; rerun passed |
| Topic | Missing scoped topic rule | Optional simple nominal topic with independent permission; reviewed borrowed prefix matching; rerun passed |
| Direct object | Missing lexical object-use binding/rule | Explicit transitive-use evidence, retained both particle findings; removed temporary collection; rerun passed |
| Unsupported applicability | Unsupported grammar checks incorrectly showed empty Pass | All grammar applicability unresolved when the construction cannot be accounted for; reviewed status precedence; rerun passed |
| Report limits/whitespace | Missing unassessed fields; whitespace would be treated as an unlisted word | Fixed report notice/limits and unresolved whitespace; clarified coverage wording; rerun passed |
| Thin runner | Missing run operation | Completed judgments versus execution error/NotRun with real adapter; reviewed executable-only JSON/error handling; rerun passed |
| Packet/error boundary | Unsupported version accepted; error cause lost | Envelope/size/count/ID validation and complete error chain; reviewed bounded reads and per-case continuation; rerun passed |

An added real-adapter isolation test passed against the existing pinned configuration
without requiring a production change. Strict Clippy identified a large upstream
error variant; boxing that cause preserves the typed error/source chain and keeps
success-path Results small. Final formatting, strict locked Clippy, all 122 tests plus rustdoc, the real
adapter probes, smoke/error demonstrations, existing preview/prepare demonstrations,
and API docs passed locally. See PLAN.md for the sandbox retry and platform limits.

Rust notes for a Ruby developer: borrowed `Sentence<'a>` prevents a report from
outliving its original text. Enums distinguish uncertainty from failure and errors,
so callers cannot confuse them with a boolean. `Box` allocates the large upstream
error only on failure; deterministic analysis stays synchronous and uses concrete
values rather than a new service/trait hierarchy.
