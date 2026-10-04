# A1 reference protocol

Version 0.3, 2026-10-03. Implementation and original synthetic fixture authoring
were explicitly approved after the initial protocol-only stage. The operating
evidence standard below preserves the accepted scope/thresholds: source-backed
claims count; model-only judgments remain provisional; disagreement stays visible.
Manual blind review by another AI provider is approved and operated by the user.
No external model has been called by the implementation agent.

Use this document with the [packet format](A1_REVIEW_PACKET.md) and the
[copyable external-model prompt](A1_EXTERNAL_REVIEW_PROMPT.md). The companion [binding sheet](A1_BLIND_PACKET_HEADER.md) specifies the implemented
boundary; [implementation status](A1_IMPLEMENTATION.md) records evidence limits.
Only the separate original synthetic draft contains cases; no learner material
or public answer key is included.

## What the investigation can establish

A1 evaluates bounded offline analysis of supplied modern Japanese sentences of
at most 100 Unicode characters. For the packet, count Unicode scalar
values, including spaces and punctuation; measure locations separately as
zero-based, half-open UTF-8 byte spans in the unchanged original text.

The accepted scope covers vocabulary identity, regular godan/ichidan polite
present/past/negative forms, narrowly scoped topic は and object を, and nominal
です. Use the linguistic term **non-past** for the present-form category; do not
infer a contextual tense from its ending alone. Preserve whole compounds and
their components. Familiar components or kanji cannot authorize a whole word.
Other grammar, multiword expressions, and reading/sense ambiguity expose limits.
There is no general grammar, naturalness, or contextual-sense validator.

The binding sheet makes those limits concrete before cases are written:

| Area | Operational boundary |
| --- | --- |
| Verb forms | Regular godan/ichidan bases with ます, ません, ました, ませんでした; dictionary form can identify the base without authorizing plain-form usage. Irregular, honorific, contracted, and other constructions remain outside this recognition claim. |
| Topic は | A simple explicit nominal topic in a single clause; no claim to resolve contrast, discourse felicity, ellipsis, or every は occurrence. |
| Object を | A simple explicit object of a source-supported transitive verb; no route, departure, or other を use inferred from the character alone. |
| Nominal です | Affirmative non-past nominal predication; adjective predicates, nominal negation/past, and other constructions require later scope. |
| Vocabulary | Explicit synthetic word identities and permissions; readings/senses stay associated, with alternatives retained. No blanket normalization, substring permission, or inferred mastery. |

These boundaries are deliberately narrower than the reference grammars. A
negative case may be valid Japanese that violates an explicit fixture permission.
An unsupported construction is a limitation, not automatically bad Japanese.
Do not design the core around detecting general grammatical errors.

An A1 `Pass` means that its declared required checks passed within this boundary.
It never means an accepted exercise, verified natural Japanese, or learner mastery.
Every report must retain that statement, even when all numerical targets are met.

The required-check inventory covers whole-word identity/permission,
regular verb form and permission, scoped particle use and permission, nominal
です and permission, and complete accounting for original text and limitations.
Freeze applicability rules with the bindings. A check that inspected the text
and found no applicable occurrence may report `Pass` with that explicit reason;
an omitted check is `NotRun`. Unknown applicability is `Inconclusive`, not an
empty success. Scope coverage prevents this convention from hiding unsupported
text. General naturalness and contextual sense are explicitly unassessed, not
missing mandatory capabilities for this bounded core.

### Vocabulary evidence versus contextual meaning

The first blind review exposed an ambiguity in the wording of version 0.2.
The following clarification records how the already excluded contextual-sense
task differs from the required vocabulary check, before any benchmark run:

- Establish the whole lexeme, base spelling/reading, and an attested supplied
  sense. A bounded Vocabulary Pass does **not** establish that this sense is the
  intended meaning in the sentence. Ordinary polysemy alone does not prevent
  this limited check from passing; retain the alternatives and the unassessed
  contextual claim. Never claim that another sense is impossible without evidence.
- Competing supplied identities, unresolved base/readings, uncertain lexical
  boundaries, and noun/adjective or verb-class alternatives that affect a required
  check remain Inconclusive. A dictionary's decision to group senses into one
  entry is not an identity rule or a way to settle ambiguity.
- If a case requires choosing a contextual meaning or resolving an idiom to
  justify its requested conclusion, keep that conclusion unsupported. A structural
  Pass cannot satisfy that challenge; the challenge safeguard is unchanged.
  A missing permission may establish Fail only for an identified supported use.
- Unknown noun-versus-adjective applicability cannot yield Nominal or Scope Pass.
  Do not hide that uncertainty inside a limitation note attached to Pass.

No fixture, binding, outcome threshold, or runtime behavior changes with this
clarification. Preserve the original v0.2 review and packet, record adjudication
under v0.3 separately, and use v0.3 consistently for later batches and holdout.
The six overall first-batch recommendations are retained; their per-check reasons
and evidence are corrected in the private ledger. Model-only assertions are not
promoted merely because the overall recommendation is supported.

## Reference evidence and analyzer results are separate

Record evidence **per decisive claim**, then summarize it per case:

| Reference evidence | Meaning | Use in the main score |
| --- | --- | --- |
| Source-backed, provisionally adjudicated | A named source was actually opened; its passage supports the exact lexical fact or rule; application to this case is explicit; no material alternative/disagreement remains unresolved. | Eligible for the bounded score; still not independently validated linguistic ground truth. |
| Model-only provisional | A model judgment, agreement, remembered rule, inaccessible citation, or source that does not substantiate the decisive claim. | Exploratory tally only; cannot establish that a target was met. |
| Disputed/unresolved | Conflicting evidence, plausible alternatives changing the outcome, or insufficient evidence to choose a reference result. | Report as a reference limitation; never relabel it to match analyzer output. |

Also count missing/not-yet-reviewed references explicitly; they have no judgment.

A source may directly state a lexical fact or provide a rule that must be applied.
Record `direct` versus `rule application` and the remaining inference. A citation
to a general grammar page is not evidence for a particular verb's class or every
word in a compound. A dictionary entry is not proof of its contextual reading.
Model agreement cannot promote a claim to source-backed. Different providers
can share training material and errors; blindness reduces answer leakage, not
correlated error. The process does not call either model independent
linguistic ground truth.

LLM-judge research documents position, verbosity, and self-enhancement biases and
reasoning limits in conversational evaluation. It does not validate this Japanese
reference set. That is the methodological reason to hide labels, preserve raw
reviews, and audit citations instead of adopting an agreement threshold.
[Zheng et al., NeurIPS 2023, abstract and paper](https://arxiv.org/abs/2306.05685v4).

For analyzer checks, keep `Pass`, `Fail`, and `Inconclusive` as completed judgments.
Keep execution errors and `NotRun` separate. An unresolved reference is neither
an analyzer `Inconclusive` nor an execution error. A model-service failure during
manual reference review is also not an analyzer failure.

## Verified starting references

The following public pages were opened and their stated sections checked on
2026-10-03. The paraphrases describe source coverage, not tested analyzer
coverage. Returned blind reviews receive separate citation audits; current
per-case coverage and remaining work are recorded in
[implementation status](A1_IMPLEMENTATION.md). Those counts measure source
coverage, not analyzer accuracy. Source sentences are not fixtures and must not
be copied into them.

| ID | Source and exact locator | Supported claim and limit |
| --- | --- | --- |
| G1 | TUFS, [Nです／Nではありません, explanation 001](https://www.coelang.tufs.ac.jp/mt/ja/gmod/contents/explanation/001.html), points 1–2 | Nominal predicate construction and polite forms. A1 covers only affirmative non-past です; the page's negative constructions do not expand scope. |
| G2 | TUFS, [NはNです, explanation 002](https://www.coelang.tufs.ac.jp/mt/ja/gmod/contents/explanation/002.html), points 1–2 | Introductory topic use and pronunciation of は. The page acknowledges multiple uses; it does not settle arbitrary discourse readings. |
| G3 | TUFS, [Vます／Vません, explanation 023](https://www.coelang.tufs.ac.jp/mt/ja/gmod/contents/explanation/023.html), points 1–3 and non-past table | Polite affirmative/negative non-past; includes habitual and future uses. Endings do not alone determine contextual tense. |
| G4 | TUFS, [Vました/Vませんでした, explanation 025](https://www.coelang.tufs.ac.jp/mt/ja/gmod/contents/explanation/025.html), points 1–3 | Formation of polite past and negative past from the polite forms. Broader aspectual interpretation is outside A1. |
| G5 | TUFS, [動詞の３種類, explanation 039](https://www.coelang.tufs.ac.jp/mt/ja/gmod/contents/explanation/039.html), points 2–4 and 5 | Group 1/2 formation rules, irregular group, and the warning that ending in -iru/-eru does not uniquely establish class. Each tested lemma still needs lexical evidence. |
| G6 | TUFS, [NをVます, explanation 027](https://www.coelang.tufs.ac.jp/mt/ja/gmod/contents/explanation/027.html), points 1–2 | Object marking and pronunciation of を in the stated construction. No arbitrary object/verb compatibility guarantee. |
| G7 | TUFS, [格助詞, explanation 053](https://www.coelang.tufs.ac.jp/mt/ja/gmod/contents/explanation/053.html), section II, points 1–4 | を also marks departure/path and can be absent in some uses. These are explicit counterbounds to a universal object-only rule. |
| L1 | EDRDG, [JMdict/EDICT project](https://www.edrdg.org/wiki/JMdict-EDICT_Dictionary_Project.html), FORMAT and LEXICOGRAPHICAL DETAILS / Inflections | Describes headwords, readings, senses, POS, and normally uninflected verb entries. This is a reference-discovery starting point, not evidence for any selected entry or contextual sense. The page calls itself dated. |

Source audit notes: G5's explanation table renders `tebe-masu`, whereas its
[card table](https://www.coelang.tufs.ac.jp/mt/ja/gmod/contents/card/039.html)
renders `tabe-masu`; retain this discrepancy rather than silently copying or
correcting a table. Use the stated rule and corroborating evidence for a future
claim. EDRDG's [old wiki URL](https://www.edrdg.org/wiki/index.php/JMdict-EDICT_Dictionary_Project)
now serves a closure notice; the table links to the accessible local copy.
These checks verify source text and locate
evidence; they are not a qualified human Japanese review.

JMdict remains manual reference material, with no A1 runtime integration or
dictionary download implied. Per-case lexical facts, whole-word boundaries,
multiword interpretations, and contextual alternatives still need their own
citations. Several websites reproducing one dictionary count as one source.

The first lexical audit used named Shogakukan dictionary sections hosted by
[Kotobank](https://kotobank.jp/): Digital Daijisen and Seisenban Nihon Kokugo
Daijiten. Their actual headwords, parts of speech, sense locators, and limitations
are recorded per claim outside Git. These substitute sources substantiate lexical
facts; they do not verify the reviewer's inaccessible Jisho pages, JMdict sequence
numbers, or contextual interpretations. Search snippets did not count as evidence.

## Practical review sequence

Implementation and synthetic case creation are authorized. The visible draft was
authored after contract tests, and that exposure is recorded. It is development
material, not blind holdout. No benchmark-driven tuning or held-out scoring has
occurred. Private holdout/reference freeze must precede that tuning.

1. **Fix the binding sheet.** Record the exact supported rules, per-case synthetic
   vocabulary permissions, and required check list. Keep free-form grammar
   declarations verbatim; bindings are explicit evaluation metadata and do not
   interpret a real learner's descriptions. Use only newly authored synthetic
   material with explicit reuse terms and provenance.
2. **Prepare references before analyzer output exists.** A first reference pass
   records claims, alternatives, source locations, and proposed outcomes. A
   second provider receives only the blind packet, in a fresh conversation,
   operated manually by the user. Hide first judgments, rationales, case category,
   partition, balance, expected outcomes, thresholds, analyzer identity and output.
   The common scope/permissions and a neutral source index are visible.
   The user can keep holdout authoring, initial reference work, and citation
   reconciliation in a separate non-implementation chat after authorization;
   its private files and answers must not be sent back to the implementer early.
   This does not require the user to adjudicate Japanese without evidence.
3. **Use small batches.** A practical schedule is batches of six, with the
   presentation order fixed before the first review.
   Use opaque IDs and a fixed shuffled order; do not disclose the batch category.
   One review per case is enough initially. Record model/version as displayed,
   date, settings if available, browsing availability, exact prompt, packet, and
   unedited response. Unknown settings stay unknown. Do not shop for agreement.
4. **Verify citations outside the reviewer's assertions.** The coordinator opens
   every decisive source, checks the publisher, section/entry, quote or paraphrase,
   restrictions and relevance, and writes a short explanation in English. Check
   contrary passages too. Never send a whole packet or sentence as a search query;
   use source titles, grammar terms, or minimal synthetic lexical lookup terms.
   Search snippets and model summaries are not verification. If interpretation
   itself remains uncertain without Japanese expertise, leave it provisional.
5. **Reconcile once, preserving both answers.** Compare only after both initial
   passes are saved. One focused clarification may ask for source evidence and
   alternatives; it is marked unblinded. No vote or third-model majority resolves
   a substantive disagreement. Resolve by documented evidence or retain unresolved
   status. Never erase the minority interpretation or overwrite the raw review.
6. **Freeze the set before tuning.** Record checksums of exact UTF-8 inputs,
   bindings, reference ledger, source locators, prompt, and partition manifest.
   Keep related cases together: minimal pairs, shared lexical ambiguities,
   compound/component contrasts, and paraphrases belong to one partition.
   Foundational rules may recur; near-duplicate realizations must not cross it.
7. **Develop, then score once.** Only development cases and references reach the
   implementer. Freeze the analyzer revision/configuration before the custodian
   reveals held-out inputs for an offline run; save outputs before joining labels.
   No fix or reference change follows inspection of a held-out result within the
   same claimed run. Later fixes use a new version and fresh held-out families.

The user is the holdout custodian. Held-out cases and their manual reviews stay
outside Git and outside the implementer's chat until scoring. Authoring/reviewing
holdout in the implementer's context and later hiding a file does not restore
blindness. If separation cannot be maintained, report a contaminated frozen test
set and withhold a claim of meeting the blind holdout gate. The external reviewer
may see the synthetic holdout for this authorized manual review; no public chat
link or publication occurs before scoring. Record any known exposure.

## Set composition and decision rules

Retain the accepted 60-case plan:

| Primary category | Development | Held-out | Challenges |
| --- | --- | --- | --- |
| Vocabulary | 6 (3 positive, 3 negative) | 6 (3 positive, 3 negative) | — |
| Inflection | 6 (3 positive, 3 negative) | 6 (3 positive, 3 negative) | — |
| Particle | 6 (3 positive, 3 negative) | 6 (3 positive, 3 negative) | — |
| Grammar | 6 (3 positive, 3 negative) | 6 (3 positive, 3 negative) | — |
| Compound/multiword expression | — | — | 6 |
| Reading/sense | — | — | 6 |

Primary category prevents double counting; record overlapping phenomena too.
The equal per-category partition allocation above operationalizes the accepted
totals. Define families before partitioning and preserve them if balancing needs
revision; never split a family merely to fill a quota. Reference annotations come
from the source review, not the analyzer's preferred tokenization or readings.

Positives require support for every required claim and complete text coverage.
Negatives require a specific supported violation with a correct reason/span;
source absence or disagreement is not enough. Cases inherently requiring general
grammar/contextual judgment belong among challenges, not determinate negatives.

**Evidence gate:** require all 48 core reference outcomes to be
source-backed and provisionally adjudicated before freezing. If this cannot be
achieved, report an evidence-limited no-go with the available exploratory results.
Do not reduce denominators, silently replace difficult cases after scoring, or
count model-only cases toward the accepted targets. Before freezing, replacements
are allowed with a retained exclusion/revision log and the same family rules.

For the 24 held-out core cases retain the accepted targets together:

- Zero false acceptance: no `Pass` for a reference-negative case.
- At most one false rejection: no more than one `Fail` for a reference-positive.
- At most two analyzer `Inconclusive` core results in total.
- At least 10/12 correctly passed positives and 10/12 correctly failed negatives.
- Zero execution errors and zero missing required checks, including `NotRun`.

Report an outcome confusion table plus a reason/span audit. A negative reached
for the wrong reason does not count as correctly rejected, even if the headline
label is `Fail`. Correctness also requires retained whole/component evidence and
original spans where required. Positive checks must not omit unsupported text.
All targets apply jointly; they cannot compensate for each other. These are
small-set engineering targets, not population error-rate or mastery estimates.

All 12 challenges must run with complete reporting and receive no unsupported
`Pass`. Use `Fail` only for a demonstrated supported violation; otherwise expose
the limitation as `Inconclusive`. An error, `NotRun`, arbitrary rejection, or
blanket `Fail` does not earn challenge success. A `Pass` against a disputed or
model-only reference is an **unsupported pass**, not a proven false acceptance;
it still blocks a go decision pending evidence. Keep these counts distinct.

Report three separate conclusions: reference coverage, analyzer performance,
and run integrity. The overall recommendation is one of:

- **Bounded provisional go:** source gate, all core targets, challenge safeguards,
  and run integrity met. Authorizes no generation or accepted exercises.
- **Analyzer no-go:** supported references expose a target/safeguard failure.
- **Evidence-limited or invalid-run no-go:** references, blindness, execution, or
  required-check coverage cannot support the decision. Describe the actual cause;
  do not attribute it to measured linguistic failure by the analyzer.

An honest documented no-go can complete the investigation. Current manual-review
coverage is recorded in [implementation status](A1_IMPLEMENTATION.md);
benchmark/held-out runs are **not performed**. Only engineering contract tests
and a separate smoke demonstration have run.

## Privacy and publication

Learner caches, identifiers, real grammar declarations, source examples, reports,
inventories, and credentials never enter these packets, Git, model chats, or web
queries. Do not derive synthetic profiles by redacting or perturbing learner data.
Packet construction has no reason to open private acceptance evidence or sync.

Keep the custody manifest and unreleased holdout outside the checkout, not merely
in an ignored repository directory. Public documentation can contain this blank
protocol, citations, prompt, and exposed original reusable development/challenge
fixtures. Held-out fixtures become eligible for publication only after scoring;
all published cases must be original and explicitly reusable. Raw
reviews, reconciliation ledgers, and detailed reports stay private unless their
release is separately authorized. Record authorship, origin, exact reuse terms,
and release state;
unresolved rights prevent release. Refer to sources with links/locators and short
paraphrases; do not publish copied textbook exercises or bulk dictionary content.

## Implementation boundary

Implementation approval was received. Use strict small Red–Green–Refactor
cycles, synchronous deterministic library logic, a concrete analyzer adapter,
real adapter tests, and a thin evaluation executable/example. No general analysis
CLI, model integration, generation, repair, quiz, or speculative framework.

The Sudachi.rs v0.6.11 / SudachiDict Core 20260723 V0 pair has now been verified
through the real adapter. Exact Git, dictionary, and built-in configuration pins,
license notices, setup, and limitations are in [implementation status](A1_IMPLEMENTATION.md).
C-mode whole units and A-mode components retain original spans. G0, preparation,
sync/status, and schema 1 remain unchanged. Required locked gates apply to code
changes; manual reference work remains separate from engineering test results.
