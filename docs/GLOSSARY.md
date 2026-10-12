# Glossary

## Current domain terms

| Term | Meaning |
| --- | --- |
| Source observations | Saved provider facts about progress; not proof of mastery |
| Knowledge policy | Rules that choose usable material from saved evidence |
| Inventory | All material allowed for an attempt, from manual input or WaniKani |
| Grammar declaration / binding | Free-text familiarity / an explicit supported rule; text does not enable rules |
| Target | An existing vocabulary or grammar ID requested for practice |
| Selection | Vocabulary sent in the prompt |
| Plan / model request | Complete offline preparation / exact bytes, hash, and options to send |
| Candidate | A generated sentence awaiting checks |

See the [module map](../ARCHITECTURE.md#current-module-map) and
[naming rules](../SPEC.md#design-vocabulary-and-composition).
Story input uses string IDs.

## Historical milestone names

| Label | Scope and status |
| --- | --- |
| 1a–1d | Status, sync, resilience, then acceptance/CI; status retired |
| G0 | First-N word preview; `preview` retired |
| Preparation slice | Source-use inspection; `prepare` retired, eligibility retained |
| A1 | Bounded analysis investigation; historical no-go, runner and analyzer retired |
| G1 | Fixed-pair sentence generation; replaced by `story` |
| G2 | Focused-context experiments; replaced by the current story path |

See [history](history/README.md) for frozen evidence and code pins.
