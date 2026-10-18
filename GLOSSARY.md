# Glossary

This records Yomibu's Ubiquitous Language: use these terms consistently in code,
tests, documentation and conversation within the same model context. Document any
legitimate differences between contexts explicitly. The retained
[SQL schema](docs/plan/schema.postgresql.sql) grounds the data terms; the
[request example](docs/plan/story-generation-request.yml) illustrates proposed use,
not final product behavior or CLI formats. Existing code may use older names.

Follow the [vocabulary agreement rule](AGENTS.md#terminology): discuss new terms or
changed meanings with the user, then record agreed definitions here before code uses them.

| Term | Meaning |
| --- | --- |
| Learner name | A learner's name with surrounding Unicode whitespace removed and at least one remaining character. Names may repeat and do not establish learner identity; the assigned UUID identifies the learner. |
| Collection | A group of source or manually entered subjects. A collection may be shared or owned by a learner. |
| Subject | One source record or manually entered item within a collection. It may produce several resolved entries or none; learner progress is recorded against it. |
| Entry | One resolved vocabulary spelling, reading and sense, or one grammar usage, belonging to a subject. |
| Learner progress source | Identifies where progress for one learner and collection comes from—an external account or manual input. Collection membership is derived from progress sources; a source can exist before progress arrives. |
| Learner progress | Saved information about a subject attributed to a learner progress source, including stage, learning dates, manual declarations and exclusions. |
| Eligibility | Whether saved learner evidence allows material to be used. |
| Inventory | All vocabulary and grammar allowed for a generation attempt, whether represented in memory or through a coherent queryable view. |
| Grammar declaration | Grammar a learner says they know. This declaration alone does not enable an automated check. |
| Grammar binding | An explicit link between a grammar entry and a supported checking rule. |
| Target | A vocabulary or grammar entry explicitly requested for practice. |
| Selection | The subset of eligible vocabulary and grammar chosen as material for generation. Unselected material remains eligible. |
| Plan | The prepared inputs and options bound together for generation. |
| Model request | The exact model-input bytes and options, excluding credentials and authentication headers. A hash identifies the request bytes. |
| Candidate | A generated or revised story evaluated as one result. A story can contain multiple scenes and blocks. |

Imported learner progress does not prove mastery. “Evidence” is explanatory language,
not a separate model concept.

See the [architecture guide](ARCHITECTURE.md).
Request documents encode identifiers as text; persisted identifiers follow the SQL schema.
