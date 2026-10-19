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
| Reference collection | A shared collection designated for reference use (`collections.is_reference`), containing ordinary subjects and entries. |
| Reference subject / reference entry | A subject or entry belonging to a reference collection; a role, not a separate entity type or table. |
| Entry reference | An accepted correspondence between entries of the same kind whose target belongs to a reference collection. References are many-to-many, including across several reference collections. |
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

Use **progress record**, not **progress entry**, for an individual learner-progress
record. **WaniKani assignment** is the provider term, translated into learner progress.
Each progress record refers to a stored subject; that subject may have no resolved
entries or reference matches yet. Entry references do not declare whole subjects
equivalent or transfer progress onto reference subjects. Uncertain matching candidates
are not accepted entry references.

## Ingestion wording

These distinguish transport, persistence and operation scope; they do not require
new entities, tables or a generic workflow framework.

| Term | Meaning |
| --- | --- |
| API page | One provider-controlled response unit within a paginated traversal. |
| Save group | A finite set of prepared data saved atomically; it need not coincide with an API page. |
| Sync / import operation | The complete requested reading and saving scope, potentially spanning several save groups. State the resource and scope rather than treating every sync as a whole-collection refresh. |
| Source update time | The provider's update time for a record, used under that provider's freshness rules. |
| Observation time | When supplied data was observed, retained only through successful persistence. It does not establish upstream freshness or create a separate Source observations entity. |
| Successful completion | All required reading and saving for the declared operation completed successfully; a group commit alone does not establish this. |

Avoid unqualified **batch** where it could mean a page, save group or complete import.
A reader is an ingestion capability; a **learner progress source** is the domain
association defined above. **ImportRecord** is not an agreed universal domain entity.

See the [architecture guide](ARCHITECTURE.md).
Request documents encode identifiers as text; persisted identifiers follow the SQL schema.
