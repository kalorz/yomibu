# Private holdout custodian handoff

Historical preparation handoff. On 2026-10-04 the separate Claude Fable
conversation reported the current 24-case set reviewed, reconciled and frozen.
Its preparation instructions below are preserved; the next step is exact input
release for the frozen offline run in [evaluation status](A1_EVALUATION_STATUS.md).
Do not start another holdout set for this run.

Use this only in a **separate conversation that is not implementing Yomibu**.
It may be model-assisted; its claims remain provisional. The user keeps its
files and messages outside Git and this implementation chat until scoring.
No learner material is needed. Do not paste held-out inputs back here early.

At handoff, the active visible set had 24 source-backed, provisionally adjudicated core
outcomes and 12 challenge references. The full reference gate still lacked the
24 private held-out core cases, and no benchmark had run. This was a custody task,
not another review of the visible cases. Save the custodian's files somewhere
private and durable; temporary handoff files are not the long-term record.

Supply that conversation the reference protocol, common binding sheet, runtime
packet format, and both the [original development/challenge draft](../tests/fixtures/a1/review-draft.json)
and [active revision 2](../tests/fixtures/a1/review-draft-v2.json) as exclusion references.
The custodian may inspect public A1 contract tests for additional exposed families.
When using a combined handoff file, its exposure inventory substitutes for
opening those tests: it must include public test literals and generated-form
families, without analyzer results. It is exclusion material, not an answer key.
Do not send private learner caches, grammar files, source examples, reports, or
credentials. A fresh context prevents direct answer leakage; it is not proof of
independent expertise or training-data independence.

```text
Act as the private reference custodian for the bounded A1 investigation described
in the supplied protocol and binding sheet. This conversation must not implement
or tune the analyzer. Use only original synthetic material with explicit CC0-1.0
reuse terms; never use or request learner material. Do not run the analyzer.

Prepare 24 private held-out core candidates: six vocabulary, six inflection,
six particle, six grammar; each category has three proposed positives and three
proposed negatives. These are proposed classifications until sources support
every decisive claim. Unsupported/general grammar or context ambiguity is not
a determinate negative. Do not weaken the accepted targets or choose cases to
match a known implementation's successes.

First define lexical/contextual families. Exclude all supplied versions of public
development/challenge families, including retired inputs, and the contract-test
families. Replacement does not erase exposure. Keep minimal pairs, shared
lexical ambiguities, compound/component contrasts, and close paraphrases together.
Foundational rules may recur; changing one word in an exposed sentence is not
automatically a new independent family. Record and resolve overlap before freeze.
Retain replacements and their reasons; do not silently discard difficult cases.

Use the exact runtime input schema, opaque IDs, fixed shuffled order, original
UTF-8 text, and explicit vocabulary/grammar permissions. Keep all proposed labels,
categories, family assignments, reference evidence, and rationales in a separate
private ledger. Preserve free-form descriptions verbatim; permissions come only
from explicit bindings. Each sentence must stay within 100 Unicode scalar values.

For each case prepare an initial reference pass with whole/component identities,
verb class/form, particle role, required-check applicability, proposed outcome,
decisive span/surface, reading/sense alternatives, and precise source claims.
Actually inspect the relevant source entry or passage when possible. Record
access, exact locator, direct evidence versus rule application, remaining inference,
and contrary evidence. A general grammar citation cannot establish every word,
transitivity claim, compound boundary, or contextual meaning. Remembered or
inaccessible sources and model-only judgments remain provisional.

Produce a blind packet containing only the common sheet and case inputs, never
the private ledger or analyzer information. The user will operate another provider
manually with the supplied blind-review prompt. Do not make provider calls yourself.
After that review, preserve its complete raw answer and every disagreement. Audit
all decisive citations by opening them. Reconcile once using evidence, or retain
unresolved status. Do not use agreement or a majority vote as linguistic truth.

Freeze only after the protocol's reference gate is met across the full core set.
The coordinator handles visible development references separately. Save exact
inputs, bindings, references, source locators, prompts, exposure/revision logs,
and SHA-256 hashes privately outside the repository. Record date and versions.
If source coverage or family separation cannot be established, report an
explicit evidence-limited no-go; do not manufacture certainty.

Provide the user with local files: exact holdout-input JSON, a separate private
reference/family/source ledger, unchanged raw reviews, and a freeze manifest with
SHA-256 hashes. Use these when file attachments are available; otherwise provide
separate clearly marked copyable blocks for the user to save privately. Never
publish them or create a public share link. Unknown model settings stay unknown.

Before scoring, tell the implementation conversation only whether custody,
reference coverage, and freeze requirements are met, plus non-revealing manifest
hashes and unresolved process issues. Do not reveal held-out sentences, words,
labels, or linguistic hints. After the implementation revision/configuration is
frozen, the user may supply inputs for one offline run. Save outputs before
joining reference labels. Any subsequent tuning needs a new run/version and fresh
held-out families. None of this certifies accepted exercises or mastery.
```
