# JMdict import and vocabulary matching

## TL;DR

**Plan only; not implemented.** See the [SQL plan](schema.postgresql.sql).

- **Import once:** load common JMdict vocabulary with English senses into a shared
  database catalogue. Keep the pinned source file and checksum.
- **Keep source identity:** a subject represents an upstream record; each vocabulary
  entry represents one spelling, reading and sense. One subject can have many entries.
  Source locators let repeat imports reuse entry IDs.
- **Match with evidence:** use spelling and reading to find candidates, then meanings
  or context to confirm senses. Preserve ambiguous input as unresolved; never invent
  reading–meaning associations.
- **Link entries:** use many-to-many reference links, allowing several dictionaries.
  Store `match_method` on each link.
- **Reuse shared matches:** adding another learner does not repeat matching. Progress
  belongs to each learner and stays attached to source subjects. Private vocabulary
  can remain in owned collections.
- **Keep generation separate:** import and matching happen explicitly, outside story
  generation.
- **Jev is optional:** evaluate it for uncertain matches before adopting it.
- **Still open:** input format and locator encoding, match acceptance and provenance,
  manual disambiguation, and handling later dictionary revisions.

The detail below preserves decisions and their reasons for future implementation.

## Reference import

Import JMdict once into a shared reference collection, independently of which
learners or WaniKani subjects have been imported. Query the database for matching;
do not scan XML for every word or import the dictionary again for each learner.
Retain the pinned source file and checksum for inspection and repeatable imports.
XML and a faithful converted format are ingestion options, not competing runtime
storage models.

The initial scope is **common vocabulary with English senses and glosses**:

- A JMdict record qualifies when any spelling or reading has a `news1`, `ichi1`,
  `spec1`, `spec2` or `gai1` priority marker. This is
  [JMdict Simplified's `common` definition][common]; XML has priority markers,
  not that computed boolean.
- For qualifying records, keep every valid spelling–reading–English-sense
  combination, including forms without their own common marker. Respect
  [reading and sense restrictions][structure], kana-only forms and inherited
  part-of-speech information.
- Exclude search-only spellings/readings (`sK`, `sk`) from normalized entries;
  preserve them in `source_data`. Keep valid kana forms such as `いらっしゃる`
  when their kanji spellings are search-only.
- Retain irregular (`iK`, `ik`), outdated (`oK`, `ok`) and rare (`rK`, `rk`) forms
  with their source labels preserved in `source_data`. Importing a form does not
  make it a preferred form for generated stories.
- Keep all parts of speech. No additional rare or archaic sense filter was chosen.

Recounting the [2026-10-12 snapshot][snapshot] with this form policy gives roughly
22,600 subjects, 55,400 vocabulary entries and 133,100 translations. With a separate
detail row for each entry, the proposed structure needs about 266,600 rows, versus
1.58 million without the common filter. These are planning estimates,
not importer acceptance counts. WaniKani coverage has not been measured. The common
filter is the agreed starting point; coverage findings can justify revisiting it.

## What each record means

| Planned table | Meaning |
| --- | --- |
| `collections` | A source catalogue. JMdict uses a shared collection with `is_reference = true` and no learner owner. Personal collections can have an owner. |
| `subjects` | An upstream record, identified within its collection by `external_id`. For JMdict this is `ent_seq`; for WaniKani it is the subject ID. Preserve source observations in `source_data`. |
| `entries`, `vocabulary_details` | One resolved spelling, reading and sense, with identity and details sharing an entry ID. Several entries may share the same spelling and reading. |
| `vocabulary_translations` | Glosses expressing that entry's sense. Several synonymous glosses do not by themselves create several entries. |
| `entry_references` | Accepted correspondences between source entries and reference entries of the same kind. |
| `learner_collections`, `learner_progress` | A learner's source account and progress observations, separate from shared vocabulary. |

A subject preserves the source's grouping; it is not a universal definition of
“one word.” Both JMdict and WaniKani can group several meanings in one record.
JMdict explicitly structures senses and their restrictions. WaniKani's accepted
reading and meaning lists do not generally specify those associations.

Keep subjects even when no vocabulary entry can yet be resolved. Moving only
`external_id` into vocabulary entries would lose the shared parent for source
observations and progress. Likewise, matching does not require merging subjects
from different sources.

## Matching source words to reference words

This applies to imported WaniKani content and user-provided vocabulary. The shared
rule is to require evidence for a reading–sense association. The exact manual-input
interface and acceptance thresholds remain open.

1. Preserve the supplied spelling, readings, meanings and available context under
   their source identity. Do not replace the original observations with a guess.
2. Find JMdict candidates by spelling and reading, respecting dictionary
   restrictions. A matching form identifies candidates, not necessarily the
   intended sense.
3. Determine which candidate senses the source meanings or user context support.
   Do not pair independent reading and meaning arrays by position, manufacture
   their Cartesian product, or copy every dictionary sense into the source.
4. Create source vocabulary entries and their translations only for supported
   associations, then record accepted reference links. One source subject can
   produce several entries whose references belong to different JMdict subjects.
5. Keep ambiguous observations in `subjects.source_data` and report them
   as unresolved. Multiple competing possibilities are not multiple confirmed
   senses. A bare user-supplied word may need a reading, meaning or explicit choice;
   the interaction for obtaining that information is still to be designed.

Reference links are **many-to-many**. One source entry may have references in
JMdict and another dictionary; many source entries may reference the same
dictionary entry. These are entry-level correspondences, not declarations that
entire source subjects are equivalent.

`match_method` belongs to each link, not to the vocabulary entry. The application
checks that a link's target belongs to a reference collection. No
reference link means “no accepted reference correspondence”; an independently
resolved source entry can still be valid without one. The table does not record
failed attempts or unresolved candidates.

## Sharing, progress and eligibility

Resolve shared source content once and reuse its accepted matches. A new learner
adding an existing WaniKani subject supplies their own progress; this does not
require repeating dictionary matching. Unchanged unresolved content stays unresolved
until an explicit retry. New or changed content or reference data can require rematching.

This sharing applies within the source's collection. User-created private content
can remain in an owned collection. Do not silently deduplicate different
collections by spelling or reading, or treat stored shared content as permission
for every learner to use it.

If Bob cannot access a WaniKani subject, Alice importing it does not remove Bob's
exclusion. Preserve Bob's progress separately from content availability.

Progress remains attached to the original source subject. A match does not mark all
of the reference dictionary's senses as known. Knowledge preparation derives
eligibility from the learner's facts and policy; storage does not make eligibility
an authoritative persisted fact. Updating one learner's progress must preserve
other learners' progress and access.

Initially, manual declarations belong to manual collections. Provider imports
replace observations only under their own `learner_collection_id`; they never
write `is_marked_known` or manual collections. Overrides on imported subjects
are deferred.

Import and matching are explicit ingestion work. Story generation consumes
prepared knowledge without implicitly importing, rematching or writing source data.

## Identity and later updates

Reimporting the same pinned snapshot must reuse collection, subject and entry IDs.
Atomically find or create shared catalogues by `(provider, key)` with no learner
owner (e.g. `wanikani` / `subjects`). Keys are stable; display names are editable.
Importers require `external_id` and reuse subjects by `(collection_id, external_id)`;
JMdict uses `ent_seq`. Manual collections may omit the key; manual subjects use
their database IDs without an external ID.

Importers supply `entries.source_locator`, unique within its subject, to identify
the source spelling, reading and sense. Choose a compact encoding for the pinned
source format. Manual entries may leave it NULL. Preserve source order in the raw
record; do not infer it from UUIDs.

Sense positions are meaningful within a snapshot, not guaranteed stable across
dictionary revisions. Reordered, split or merged senses need explicit reconciliation
before existing identities and links can be reused. An automatic update service or
general history framework is not required now.

## Possible role for Jev

Jev means **TypeSafe AI's model**, not the Japanese educational vocabulary database.
It is an optional experiment, not a required dependency or an agreed final matcher.

Code should parse JMdict and shortlist structurally valid candidates. Jev could
then judge whether the supplied meanings support each candidate sense, allowing
supported, unsupported or insufficient-evidence outcomes. Do not force one winner
or treat a confidence score as proof. Preserve the distinction between dictionary
facts and inferred matches.

Before adopting it, compare it with a deterministic baseline on manually checked
examples, measuring false accepted matches as well as resolved coverage. Accepted
results should be reusable; no model call is needed merely because another learner
adds the same shared content.

## Implementation boundaries and open choices

Follow the [architecture guide](../../AGENTS.md#architecture).

Before implementing the relevant slice, settle:

- The input snapshot/format and encoding of source locators for repeat imports.
- Match acceptance rules, evidence/provenance, and the treatment of unresolved or
  rejected attempts; `match_method` alone does not answer these questions.
- The manual vocabulary input and disambiguation flow.
- A later dictionary-update reconciliation policy when updates become necessary.

A useful coverage check compares complete WaniKani vocabulary against full and
common JMdict candidate sets. Count subjects losing some or all candidates, and
inspect lost senses; “at least one candidate” is not proof of semantic coverage.

[common]: https://github.com/scriptin/jmdict-simplified#full-common-only-with-examples-and-language-specific-versions
[snapshot]: https://github.com/scriptin/jmdict-simplified/releases/tag/3.6.2%2B20261012204142
[structure]: https://www.edrdg.org/jmdict/jmdictart.html
