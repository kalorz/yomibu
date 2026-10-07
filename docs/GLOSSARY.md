# Glossary

## Current domain terms

| Term | Meaning |
| --- | --- |
| Source observations | Saved provider facts about progress; not proof of mastery |
| Knowledge policy | Rules that choose usable material from saved evidence |
| Inventory | All material allowed for an attempt, from manual input or WaniKani |
| Grammar declaration / binding | Free-text familiarity / an explicit supported rule; text does not enable rules |
| Target | An existing vocabulary or grammar ID requested for practice |
| Selection | Vocabulary sent in the prompt; checks still use the full inventory |
| Plan / model request | Complete offline preparation / exact bytes, hash, and options to send |
| Candidate | A generated sentence awaiting checks |
| Analysis | Morphological evidence: units, readings, and original UTF-8 spans |
| C/A modes | Sudachi whole units / finer components; components cannot authorize a word |
| Evaluation | Constraint judgments and findings |
| Story assessment | Evaluation plus target observations and departures from the prompt selection |
| Pass / Fail / Inconclusive | Completed judgments; execution errors and NotRun are separate |
| Lexical baseline | Token-overlap vectors used for demos; no understanding of meaning |

See the [module map](../ARCHITECTURE.md#current-module-map) and
[naming rules](../SPEC.md#design-vocabulary-and-composition).
Analyze uses one-based grammar IDs; story input uses string IDs.

## Historical milestone names

| Label | Scope and status |
| --- | --- |
| 1a–1d | Status, sync, resilience, then acceptance/CI; active capabilities |
| G0 | First-N word preview; `preview` retired |
| Preparation slice | Source-use inspection; `prepare` retired, eligibility retained |
| A1 | Bounded analysis investigation; historical no-go, runner retired, analyzer active |
| G1 | Fixed-pair sentence generation; replaced by `story` |
| G2 | Focused-context experiments; replaced by the current story path |

See [history](history/README.md) for frozen evidence and code pins.
