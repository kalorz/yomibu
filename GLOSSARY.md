# Glossary

Use these terms consistently in code, documentation and conversation. Definitions
follow the [SQL plan](docs/plan/schema.postgresql.sql) and
[story request](docs/plan/story-generation-request.yml); existing implementation
names may still reflect older behavior.

| Term | Meaning |
| --- | --- |
| Collection | A group of source or manually entered subjects. A collection may be shared or owned by a learner. |
| Subject | One source record or manually entered item within a collection. It may produce several resolved entries or none; learner progress is recorded against it. |
| Entry | One resolved vocabulary spelling, reading and sense, or one grammar usage, belonging to a subject. |
| Learner progress | Saved observations, declarations and exclusions for a learner and subject, from which eligibility is derived. |
| Source observations | Saved progress data from a provider. Progress does not prove mastery. |
| Eligibility | Whether saved learner evidence allows material to be used. |
| Inventory | All vocabulary and grammar allowed for a generation attempt, whether represented in memory or through a coherent queryable view. |
| Grammar declaration | Grammar a learner says they know. This declaration alone does not enable an automated check. |
| Grammar binding | An explicit link between a grammar entry and a supported checking rule. |
| Target | A vocabulary or grammar entry explicitly requested for practice. It must be eligible, satisfy applicable filters and fit within the selection limits. |
| Selection | The subset of eligible vocabulary and grammar chosen as material for generation. Unselected material remains eligible. |
| Plan | The prepared inputs and options bound together for generation. |
| Model request | The exact model-input bytes and options, excluding credentials and authentication headers. A hash identifies the request bytes. |
| Candidate | A generated or revised story evaluated as one result. A story can contain multiple scenes and blocks. |

See the [architecture guide](ARCHITECTURE.md).
Request documents encode identifiers as text; persisted identifiers follow the SQL schema.
