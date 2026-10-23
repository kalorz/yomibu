# Glossary

Agreed domain meanings, Yomibu's ubiquitous language. The [SQL schema](docs/plan/schema.postgresql.sql) grounds the
data terms. Follow the [domain language rules](ARCHITECTURE.md#domain-language) and
agree new or changed terms first, as [AGENTS.md](AGENTS.md#terminology) describes.

| Term | Meaning |
| --- | --- |
| Learner name | A learner's name with surrounding Unicode whitespace removed and at least one remaining character. Names may repeat and do not establish learner identity; the assigned UUID identifies the learner. |
| Collection | A group of source or manually entered subjects. A collection may be shared or owned by a learner. |
| Subject | One source record or manually entered item within a collection. It may produce several resolved entries or none; learner progress is recorded against it. |
| Entry | One resolved vocabulary spelling, reading and sense, or one grammar usage, belonging to a subject. |
| Reference collection | A shared collection designated for reference use (`collections.is_reference`), containing ordinary subjects and entries. |
| Reference subject / reference entry | A subject or entry belonging to a reference collection; a role, not a separate entity type or table. |
| Entry reference | An accepted correspondence between entries of the same kind whose target belongs to a reference collection. References are many-to-many, including across several reference collections. A reference does not make whole subjects equivalent or transfer progress between them. |
| Learner progress source | Identifies where progress for one learner and collection comes from: an external account or manual input. Collection membership is derived from progress sources; a source can exist before progress arrives. |
| Learner progress | Saved information about a subject attributed to a learner progress source, including stage, learning dates, manual declarations and exclusions. |
| Progress record | One learner progress record for one subject. |
| Eligibility | Whether saved learner evidence allows material to be used. |
| Inventory | All vocabulary and grammar allowed for a generation attempt, whether represented in memory or through a coherent queryable view. |
| Grammar declaration | Grammar a learner says they know. This declaration alone does not enable an automated check. |
| Grammar binding | An explicit link between a grammar entry and a supported checking rule. |
| Target | A vocabulary or grammar entry explicitly requested for practice. |
| Selection | The subset of eligible vocabulary and grammar chosen as material for generation. Unselected material remains eligible. |
| Plan | The prepared inputs and options bound together for generation. |
| Model request | The exact model-input bytes and options, excluding credentials and authentication headers. A hash identifies the request bytes. |
| Candidate | A generated or revised story evaluated as one result. A story can contain multiple scenes and blocks. |

## Sync and import wording

| Term | Meaning |
| --- | --- |
| API page | One provider-controlled response unit within a paginated traversal. |
| Save group | A finite set of prepared data saved atomically; it need not coincide with an API page. |
| Sync / import operation | The complete requested reading and saving scope, potentially spanning several save groups. State the resource and scope rather than treating every sync as a whole-collection refresh. |
| Source update time | The provider's update time for a record, used under that provider's freshness rules. |
| Observation time | When supplied data was observed, retained only through successful persistence. It does not establish upstream freshness. |
| Successful completion | All required reading and saving for the declared operation completed successfully; a group commit alone does not establish this. |

## Composition wording

| Term | Meaning |
| --- | --- |
| Operation | A behaviour app exposes to driving crates, run as steps in a fixed order. Every driving crate calls the same operations. |
| Step | One step of an operation, run in the operation's fixed order. A step may call a capability or be plain application code. |
| Capability | A kind of replaceable behaviour, defined as one port trait in core, such as selector, generator or assessor. Operations call capabilities; components implement them. |
| Component | A named implementation of one capability with its own typed options, selectable in configuration. |
| Composition | The ordered choice of components per capability for one operation, taken from the default configuration or supplied with a request. |
