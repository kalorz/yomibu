# Supplied-text analysis

`analyze` examines word forms and runs a few checks on one sentence, offline.
It does not prove naturalness, meaning in context, or exercise acceptance.

## Run the fixtures

For normal use, [install a managed dictionary](DICTIONARY.md#install-and-use-offline).
To use the developer bundle directly:

```sh
python3 scripts/setup_test_dictionary.py
cargo run --release --locked -- analyze --dictionary target/test-resources/sudachi-core/current/system_core.dic --input tests/fixtures/analyze/nominal.json
```

Setup is an explicit download. Analysis never downloads. Use `--release` for faster
startup. Add Cargo's `--offline` before `--` after dependencies are cached.
`nominal.json`, `unlisted.json`, and `object.json` produce Pass, Fail, and Inconclusive.

## Input and supported checks

```json
{
  "version": 1,
  "sentence": "犬です。",
  "grammar": ["です — manual familiarity"],
  "bindings": {
    "vocabulary": [
      {"written_form": "犬", "reading": "イヌ", "sense": "dog", "direct_object": false}
    ],
    "grammar": [{"declaration_id": 1, "rule": "NominalDesu"}]
  }
}
```

Fields are required; unknown fields are errors. Reject malformed JSON/UTF-8,
unsupported versions, blank declarations and invalid bindings. Stop reading at
64 KiB plus one byte to reject oversized files before decoding. Sentence text must
be nonblank and at most 100 Unicode scalars; preserve it without trimming or normalization.
Empty arrays grant no permissions. Grammar IDs are one-based positions;
descriptions preserve order and duplicates and never enable rules.
Readings use katakana. Sense and transitive-use labels are user claims, not validation.

| Rule | Supported form |
| --- | --- |
| `NominalDesu` | Nominal です |
| `TopicWa` | Scoped topic は |
| `ObjectWo` | Scoped object を; combination remains unresolved |
| `PoliteNonPast` | Regular ます |
| `PolitePast` | Regular ました |
| `PoliteNegativeNonPast` | Regular ません |
| `PoliteNegativePast` | Regular ませんでした |

Five checks cover vocabulary, inflection, particles, grammar, and scope. Recognition
covers two simple patterns: nominal です or a regular polite verb, with optional
topic/object and final 。. Extra clauses or unsupported forms expose scope limits.
Object/predicate combinations stay Inconclusive unless a known permission failure
makes the overall result Fail.

## Output and execution behavior

Text lists checks, reasons, and UTF-8 byte ranges in the original sentence.
The start of a range is included; the end is excluded.
`--json` includes exact input, analysis/provenance, outcome, and evaluation with
`basis: explicit_word_uses`.
Pass, Fail, and Inconclusive all exit zero. Input/execution errors exit nonzero,
write stderr, and publish no evaluation. Untrusted terminal text is escaped.
JSON version 1 retains decoded strings and original spans; presentation escaping
does not change them. No structured error report is defined.

Analysis reads no learner state or credentials and performs no writes or network
calls. Explicit `--dictionary PATH` needs no HOME; managed defaults use their own root.
It ignores `--data-dir` and starts no runtime. Diagnostics preserve trusted line
breaks and indentation while escaping supplied values; help/usage names `yomibu`.

## Library and evidence

Compose `SudachiAnalyzer`, `Sentence`, and `evaluation::evaluate` directly.
See [SPEC](../SPEC.md#bounded-analysis-and-evaluation) for the full contract,
[dictionary policies](DICTIONARY.md), and [fixtures](../tests/fixtures/analyze/README.md).
The analyzer remains active despite A1's [historical no-go](history/README.md#analysis-investigation).
