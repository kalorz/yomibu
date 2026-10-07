# A1 evaluation status

2026-10-04. **A1 investigation complete: no-go for this frozen implementation.**
Held-out core targets are met; three unsupported visible challenge Pass results
fail the separate safeguard. Reference review and private scoring are complete on
the visible audit records and the separate custodian's receipts. All references remain model-assisted and
provisional. Hidden source/answer files have not been inspected here; reported
hashes are not a substitute for that distinction. Model agreement is not
independent linguistic ground truth.

The 24 visible core references plus the custodian's 24 held-out references meet
the reported 48-core source gate. All 12 challenge references are also ready.
The reference freeze records exact inputs, ledgers, prompts, source provenance,
revision/exposure history and hashes before evaluation. The custodian reports
replacement of the five exposed families; the retired review receives no credit.
Unknown reviewer model/version/settings remain unknown. The actual-service
records distinguish the Claude initial author from ChatGPT/Gemini reviewers.

## Visible development and challenge results

The real pinned adapter and thin executable ran offline on all 36 active visible
cases. Raw output was saved and hashed before joining the private reference key.
There were no execution errors or missing checks. Exact input text, original byte
spans and C/A token/component evidence were preserved. These are exposed
development/challenge results, not held-out accuracy estimates.

| Measure | Result after the reporting fix |
| --- | --- |
| Development positive outcomes | 12/12 Pass |
| Development negative outcomes | 12/12 Fail |
| Development negative reason and exact decisive span | 12/12 match |
| Development required-check judgments | 120/120 match |
| Challenges | 1 Fail, 8 Inconclusive, 3 Pass |
| Challenge outcome agreement | 8/12 |
| Challenge required-check judgment agreement | 38/60 |
| Unsupported challenge Pass | **3 — safeguard failed** |
| Execution errors / missing required checks | 0 / 0 across all 36 cases |

The three unsupported Pass results concern multiword-expression ambiguity that
the structural analyzer does not resolve. Another challenge's whole compound was
split by the pinned analyzer, yielding Inconclusive instead of the reference
permission failure. Some unresolved lexical/POS cases also received overconfident
individual checks despite an overall Inconclusive result. Detailed examples,
reason/span comparisons, disagreements and reports remain outside Git.

The first visible run already matched all 24 development outcome labels, but
four negative findings highlighted the entire inflected verb instead of the
decisive polite ending. A regression test failed with `0..12` versus `6..12`.
The production fix uses the end of the existing stem span as the finding start.
Focused Green and post-refactor-review tests passed for all four polite endings.
Review of borrowing, naming, duplication and modelling justified no further
refactor. The second run retains all outcome/check judgments and corrects the
four decisive spans. Both runs and the original references remain preserved;
the held-out input was unseen throughout this development change.

Required post-change gates passed: `cargo fmt --check`, strict locked all-targets
and all-features Clippy, and locked `cargo test --all`, using offline dependencies.
The initial full-test attempt could not bind local mock-server ports in the
sandbox; the permitted retry passed. No real learner data, credentials, source
sync, downloads or external model calls were involved.

## Held-out run and conclusion

The implementation/configuration is frozen after the reporting fix and gates,
with the base Git commit, exact changed/untracked build-file manifest, source
archive, executable, toolchain, upstream revision, dictionary and configuration
hashes recorded privately. The checkout's uncommitted state is explicit; the
base commit alone does not identify the frozen executable.

The 24-case held-out input was released on 2026-10-04. Its exact bytes matched
the recorded freeze hash, and all 58 build files, the saved executable,
dictionary, configuration and reference freeze matched before execution. The
existing frozen executable ran **once offline**, without rebuilding or tuning.
Complete stdout and stderr were saved and hashed before inspection or scoring.
Integrity checks found 24 preserved inputs, 120 completed required checks, zero
execution errors, zero NotRun cases, and valid original UTF-8/C/A spans with
matching provenance. These checks establish report completeness, not accuracy.

The user returned a non-revealing scoring receipt from Claude Fable's existing
“8 FILES” conversation. The downloaded input, complete output and handoff receipt
match their previously recorded hashes. The scoring receipt quotes the same
input/output, implementation-freeze and private-ledger hashes. All 24 echoed
inputs remain unchanged and in order; all 120 check states are complete. The
private ledger and scoring record were not opened here: the comparisons below
are the custodian's reported scoring, not an independent rescore.

| Held-out core measure | Reported result | Target |
| --- | --- | --- |
| Reference positives | 12 Pass, 0 Fail, 0 Inconclusive | At least 10/12 correctly passed |
| Reference negatives | 12 Fail, 0 Pass, 0 Inconclusive | Zero false acceptance |
| False rejection | 0 | At most 1 |
| Analyzer Inconclusive | 0 | At most 2 |
| Negative failing check / reason class | 12/12 match | Correct reason required |
| Negative exact decisive span | 11/12 match | At least 10/12 correct negatives including reason/span |
| Required-check judgment agreement | 120/120 | All five checks present |
| Execution errors / missing checks / NotRun | 0 / 0 / 0 | Zero |

**All held-out core targets are met using the strict exact-span count of 11/12.**
The remaining span is a strict sub-span of its frozen reference. The custodian
reports a whole-predicate reference where comparable cases specify the ending;
that explanation has not been independently adjudicated here. Preserve the
discrepancy and unchanged reference; do not award an exact match. The receipt also
notes varying coverage wording in failed particle checks and that base-reading
matching is not directly observable from the emitted surface readings.

Run evidence and reference evidence remain separate. The custodian independently
checked the output and its ledger, but took executable/dictionary/configuration
and offline-run claims from this side's receipt. Those were verified here before
the run. At closure the original temporary archive is no longer available; the
user's downloaded input, output and handoff receipt remain intact and were
rechecked. The archived executable and full build manifest were not reverified at
closure. The private ledger's unchanged hash and the append-only custody-log
updates are custodian attestations. The author of the references also performed
the private scoring. These are small-set provisional findings, not population
error-rate, mastery or independent linguistic-validity estimates.

The completed investigation records **no-go** because the visible challenge
safeguard failed. Held-out success does not override its three unsupported Pass
results. No implementation, reference, threshold or fixture was changed after
scoring, and the analyzer was not rerun. Prior locked engineering gates remain
the applicable code checks; closure changed only aggregate documentation.
Input, output, private ledgers and detailed receipts remain outside Git; scoring
does not itself release held-out fixtures for publication.

Any follow-up must be a separately recorded implementation revision addressing
the exposed challenge limitations. This scored holdout cannot be represented as
unseen evidence for later tuning. No sentence is an accepted exercise. Generation
and other deferred product work remain outside this completed investigation.

Rust note for a Ruby developer: `start..end` is a half-open byte range into the
original string. Narrowing its start changes the reported location without
copying or editing the sentence or changing the grammatical judgment.
