# Grammar import and reference matching

## TL;DR

**Plan only; NINJAL import is not implemented.** Read with the
[SQL plan](schema.postgresql.sql) and [vocabulary plan](jmdict-import-and-matching.md).

- **Start with NINJAL:** import a pinned Nihongo Bunkei Database snapshot into a
  shared reference collection. Retain its source file, checksum and attribution.
- **Preserve usages:** a subject is an upstream record; each grammar entry is one
  resolved sense or usage, with its own explanations, formations and examples.
- **Use the shared model:** `entries` owns identity; `grammar_details` supplies
  grammar fields. Each entry has exactly one matching detail row.
- **Keep Japanese content:** English translations are optional. Preserve source
  distinctions and additional information rather than inventing missing content.
- **Match specific usages:** a shared headword is only a candidate. Record accepted
  correspondences in `entry_references`; preserve unresolved input.
- **Keep progress individual:** shared content and matches are reusable. Manual
  collections use the same model, without an external account.
- **Separate explanations from checks:** imported grammar does not automatically
  become executable assessment rules or known learner material.
- **Still open:** source identifiers and locator encoding, detailed field mapping,
  description categories, and matching/disambiguation rules.

## Reference source and scope

Use NINJAL's **Nihongo Bunkei Database** (日本語文型データベース) as the initial
reference source. The research preferred its named editorial team and structured
content over dataset size alone. See the [project and contributors][ninjal-about]
and [official dataset][ninjal-data].

The research examined the **2026.01 edition**, distributed as XML in a ZIP under
CC BY 4.0, with Japanese explanations. It reported 800 records, 958 senses,
1,988 connection patterns and 9,552 examples. The inspected [Yomitan conversion][conversion]
confirmed 800 distinct entry IDs; the finer counts came from its conversion report.
Verify the original XML and counts when choosing the import snapshot.

Import the reference collection independently of learners. No common-word filter,
English-only filter or difficulty cutoff has been selected for grammar.
[OpenJLPT][openjlpt] remains a candidate for English content, not an additional
source required for the first importer. Later translations need their own provenance.

Record the source URL, licence, attribution, revision and checksum on `collections`.
Retain the downloaded artifact and preserve each source record in `subjects.source_data`.
Collection metadata describes the current dataset; it is not revision history.

## Mapping into the shared schema

The table below describes the proposed NINJAL import's use of the shared schema.
`collections` remains provider-independent and also holds other source and manual collections.

| Planned table | Use in this grammar import |
| --- | --- |
| `collections` | One shared NINJAL collection: `is_reference = true`, no learner owner. |
| `subjects` | One upstream record, with a collection-scoped `external_id` and normalized `kind = grammar`. |
| `entries`, `grammar_details` | One resolved usage, sharing an entry ID. Details contain the headword, optional reading and explicit assessment-rule bindings. |
| `grammar_descriptions` | Explanations in their source language, with purpose, position and origin. |
| `grammar_formations` | Ordered connection patterns belonging to a particular usage. |
| `grammar_examples` | Examples belonging to a usage, optionally linked to one of its formations. Translation and translation language can be absent. |
| `entry_references` | Accepted correspondences between grammar entries, with a method recorded per link. |

An entry already represents a sense or usage; no separate sense table is needed.
Several entries can have the same headword. For example, appearance and hearsay
uses of `そうだ` remain distinct even if a source groups them together.

Formation patterns describe how a construction connects to surrounding words.
Preserve their example associations: an example of one formation must not become
evidence for another. Keep formations as rows because examples reference them.
Before deleting a formation, explicitly reassign or delete its examples.
Descriptions and examples also retain their metadata rather than becoming text arrays.

The application must publish each entry and its required detail row atomically.
The SQL constraints enforce matching kinds and formation ownership; they do not
ensure that every parent entry has a detail row. The `material_kind` enum covers
vocabulary and grammar. Provider identifiers and assessment-rule codes remain open
text values; description-purpose categories are not yet fixed as an enum.

## Preserve meaning during normalization

Map meanings, usage explanations, restrictions and contrast notes to appropriate
description purposes. Their exact categories and mapping remain an importer decision.
Do not rely on position alone to distinguish their roles.

Preserve furigana, lookup variants, source-wide explanations, example context/notes,
related expressions and other source details in `source_data`. Decide which need
normalized representation using real fixtures and the consuming workflow. Raw
preservation prevents loss; it does not automatically expose those distinctions
through the common model.

Keep difficulty classifications and review metadata with their source meaning.
NINJAL difficulty must not silently become JLPT level. Add normalized fields only
when a concrete selection or review rule needs them, retaining both scheme and value.

## Matching and manual collections

Use the same separation between source content and accepted reference links as
the [vocabulary matching plan](jmdict-import-and-matching.md#matching-source-words-to-reference-words).
For grammar, compare usage meaning, formation restrictions and supporting examples;
matching headwords alone cannot establish correspondence.

Preserve user-provided descriptions and source identity. Ambiguous declarations,
such as an unspecified use of `そうだ`, may need clarification. Keep unresolved
observations in `subjects.source_data`; do not silently expand them into every
reference usage. A resolved manual entry can exist without a dictionary match.

Links are many-to-many and connect entries of the same kind. The application checks
that the target collection is a reference collection. Correspondence between usages
does not make entire source subjects equivalent or establish interchangeability of
all their formations. The acceptance criteria and manual disambiguation interface
remain open. No AI matching model has been selected for grammar.

Manual collections can be owned or shared. `learner_progress_sources` needs no external
account for them; `learner_progress` records each source's learner progress against
subjects, including manual declarations. Create a separate manual subject for each
item that needs independent progress. Adding content does not mark it known.
Reference matches also do not grant knowledge of additional usages. Follow the shared
[progress ownership rules](jmdict-import-and-matching.md#sharing-progress-and-eligibility).

Reuse accepted matches for unchanged shared content across learners. Keep their
progress independent, enforce access to private collections, and derive eligibility
from learner evidence and policy.

## Identity, assessment and execution

Use the [repeat-import identity rules](jmdict-import-and-matching.md#identity-and-later-updates):
target the existing collection and store each usage's locator in `entries.source_locator`.
Verify the original source's record identifier; do not assume a conversion's display
label is a stable key. Choose a compact locator encoding. Sense or formation positions
are scoped to a snapshot; later revisions require reconciliation before reusing identities.

Keep `assessment_rule_codes` empty until an imported usage is explicitly mapped to
a supported checking rule. A formation string is descriptive notation, not a parser
or proof that generated text uses the construction correctly. Learner declarations,
dictionary matching and automated assessment remain separate evidence.

Follow the [architecture guide](../../AGENTS.md#architecture). Generation consumes
prepared knowledge without implicitly importing or rematching grammar.

## First implementation slice and open choices

Start with small pinned NINJAL fixtures that exercise multiple usages, several
formations, examples, missing translations and additional source notes. Confirm:

- The source identifier and locator strategy makes repeat imports reuse IDs.
- Descriptions and examples retain their usage and formation associations.
- Unmapped information remains inspectable without invented translations or checks.
- Shared content supports independent learner progress and manual declarations.

Settle description categories and handling of source-wide explanations and example
context during this mapping exercise. Matching acceptance, manual disambiguation and
later dictionary updates are separate decisions. Do not add a generic importer
framework, grammar compiler or automatic translation service for this first slice.

The SQL structure has been exercised on PostgreSQL with pgvector. An actual NINJAL
import and its linguistic mapping have not been verified yet.

[ninjal-about]: https://www2.ninjal.ac.jp/bunkeibank/about.html
[ninjal-data]: https://repository.ninjal.ac.jp/records/2000610
[conversion]: https://github.com/W1ght/ninjal-bunkei-yomitan
[openjlpt]: https://github.com/evanclan/OpenJLPT
