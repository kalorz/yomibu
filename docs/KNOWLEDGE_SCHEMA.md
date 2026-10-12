# Agreed knowledge schema target

The supplied PostgreSQL reference below is preserved verbatim. It is a design
target, not an executable migration or a description of the current SQLite store.
Grammar, examples, embeddings and automatic vocabulary resolution remain outside
this library persistence slice.

The subsequent discriminator refinement uses generated constant `material_type`
columns for vocabulary and grammar, retaining their composite subject FKs. It
replaces the reference's fixed DEFAULT/CHECK, not profile availability.

## Reconciliation

This compares the pre-correction PR with the target and the current correction.
PK/FK/UNIQUE/NOT NULL enforce relationships. SQLite uses INTEGER for identity keys
and booleans, and canonical UTC TEXT for timestamps. Content checks stay in Rust.
Scope IDs can be supplied explicitly, as PostgreSQL BY DEFAULT identity permits.
Names and optional ownership are explicit caller facts; ownership is bound to the
collection. This slice does not enforce private-collection access permissions.

| Pre-correction representation | Agreed representation | Correction or scope status |
| --- | --- | --- |
| `learners.learner_id` TEXT PK; no name | `learners.id` generated numeric PK; required `name` | INTEGER PK and caller-supplied name restored. |
| `source_collections.collection_id` TEXT PK, `source_kind`; no name/owner | `id`, `provider`, `name`, `owner_learner_id` FK RESTRICT; owner index | Names, numeric key, owner FK/index restored. |
| `source_subjects.subject_id` INTEGER PK, `collection_id`, `external_id`; UNIQUE collection/external and id/collection; collection FK RESTRICT | `id`, `source_collection_id`, `external_id`, required `material_type`, `source_data`; extra UNIQUE id/type | Numeric key, names, type/JSON source data and UNIQUE id/type restored. The user permits NULL for genuinely unknown type; vocabulary observations establish vocabulary. Collection FK remains RESTRICT. |
| `source_learner_profiles.profile_id` TEXT PK, learner/collection/account/deadline; UNIQUE learner/collection/account and id/collection; parent FKs RESTRICT | Numeric `id`, `learner_id`, `source_collection_id`, `external_account_id`; UNIQUE learner/collection and id/collection | Numeric key, names and one profile per learner/collection restored, including NULL accounts. Import timestamps/account data are not in the common publication API; raw provider facts remain in source storage. `access_expires_at` is the later explicit addition; preparation enforces it. |
| Extra `source_profile_subjects`: profile/subject PK, collection, ordered position, mapped `content_kind`; UNIQUE profile/position; profile CASCADE, subject RESTRICT | No equivalent table | Retained to preserve membership without progress, ordering, and per-profile unavailable/non-vocabulary observations. It is not subject `material_type`. |
| `source_learner_progress`: profile/subject PK, known/milestone/hidden facts; association FK CASCADE | Same PK/common facts, collection-qualified profile CASCADE and subject RESTRICT FKs; hidden default FALSE; subject index | Names, collection column, direct composite FKs/default/index restored. Association FK additionally retains publication membership lifetime. Stage/raw evidence stays in source storage. The mapped contract supplies no recorded time; `recorded_at` is omitted rather than fabricated. |
| `source_vocabulary`: subject PK, collection/word UNIQUE, written form/direct-object evidence; subject CASCADE | `vocabulary_entries`: own generated id; many entries per subject; one spelling/reading, parts of speech, primary/mapping metadata, generated type + subject FK RESTRICT | Aggregate removed. `vocabulary_entries` restores all entry fields, numeric id, many-entry cardinality, generated type/composite FK RESTRICT and subject/form indexes. SQLite stores ordered parts of speech in child rows (entry/position PK, entry FK CASCADE). |
| `source_vocabulary_readings`: subject/position PK, reading; aggregate FK CASCADE | One reading per vocabulary entry | Aggregate readings removed. Resolved entries require explicit reading/translation association evidence; independent source alternatives remain unresolved JSON observations. |
| `source_vocabulary_meanings`: subject/position PK, meaning; aggregate FK CASCADE | `vocabulary_translations`: own id, entry FK CASCADE, language/text/order/primary/origin; UNIQUE entry/language/order | Aggregate meanings removed. Numeric translation id, entry FK CASCADE, all fields and UNIQUE entry/language/order restored. Language, primary flags and origin must be supplied, never inferred from WaniKani arrays. |
| No examples, grammar or embeddings tables | `vocabulary_examples`; `grammar_entries`, `grammar_descriptions`, `grammar_examples`; `embedding_profiles`, `vocabulary_embeddings`, `grammar_embeddings` | Deferred scope. Their reference PKs/FKs/uniqueness/delete actions are preserved below; no empty runtime tables added. |

Unresolved vocabulary now stays in `source_subjects.source_data`, with its common
unpaired projection and original subject observation. Reads use the common projection
without invoking a provider mapper. It creates no entries or translations.
`MappedContent` reports it explicitly as unresolved. Existing generic inventory
preparation can use those alternatives without claiming resolved senses.
Only caller-supplied entry-level associations create entries and translations.
Entry IDs determine order within a subject; translations use language/display order.
No dictionary resolver is implemented. This slice does not implement the full target.

## Original reference

```sql
-- PostgreSQL reference schema. Requires the pgvector extension on the server.
-- Content validation and private-collection access policies belong to the application.
-- VECTOR is dimensionless here because embedding profiles can use different dimensions.
-- Long descriptive TEXT is not a unique key: PostgreSQL B-tree keys have size limits.
-- Exact-text deduplication belongs to content validation; ordering remains unique.
BEGIN;

CREATE EXTENSION IF NOT EXISTS vector;

CREATE TABLE learners (
    id BIGINT GENERATED BY DEFAULT AS IDENTITY PRIMARY KEY,
    name TEXT NOT NULL
);

CREATE TABLE source_collections (
    id BIGINT GENERATED BY DEFAULT AS IDENTITY PRIMARY KEY,
    provider TEXT NOT NULL,
    name TEXT NOT NULL,
    owner_learner_id BIGINT
        REFERENCES learners(id) ON DELETE RESTRICT
);

CREATE INDEX source_collections_owner
    ON source_collections(owner_learner_id);

CREATE TABLE source_subjects (
    id BIGINT GENERATED BY DEFAULT AS IDENTITY PRIMARY KEY,
    source_collection_id BIGINT NOT NULL
        REFERENCES source_collections(id) ON DELETE RESTRICT,
    external_id TEXT NOT NULL,
    material_type TEXT NOT NULL,
    source_data JSONB,

    UNIQUE (source_collection_id, external_id),
    UNIQUE (id, source_collection_id),
    UNIQUE (id, material_type)
);

CREATE TABLE source_learner_profiles (
    id BIGINT GENERATED BY DEFAULT AS IDENTITY PRIMARY KEY,
    source_collection_id BIGINT NOT NULL
        REFERENCES source_collections(id) ON DELETE RESTRICT,
    learner_id BIGINT NOT NULL
        REFERENCES learners(id) ON DELETE RESTRICT,
    external_account_id TEXT,
    last_import_started_at TIMESTAMPTZ,
    last_import_completed_at TIMESTAMPTZ,
    account_data JSONB,

    UNIQUE (learner_id, source_collection_id),
    UNIQUE (id, source_collection_id)
);

CREATE INDEX source_learner_profiles_collection
    ON source_learner_profiles(source_collection_id);

CREATE TABLE source_learner_progress (
    source_learner_profile_id BIGINT NOT NULL,
    source_subject_id BIGINT NOT NULL,
    source_collection_id BIGINT NOT NULL,
    progress_stage TEXT,
    is_marked_known BOOLEAN,
    learning_started_at TIMESTAMPTZ,
    first_passed_at TIMESTAMPTZ,
    is_hidden BOOLEAN NOT NULL DEFAULT FALSE,
    recorded_at TIMESTAMPTZ NOT NULL,
    source_data JSONB,

    PRIMARY KEY (source_learner_profile_id, source_subject_id),

    FOREIGN KEY (source_learner_profile_id, source_collection_id)
        REFERENCES source_learner_profiles(id, source_collection_id)
        ON DELETE CASCADE,

    FOREIGN KEY (source_subject_id, source_collection_id)
        REFERENCES source_subjects(id, source_collection_id)
        ON DELETE RESTRICT
);

CREATE INDEX source_learner_progress_subject
    ON source_learner_progress(source_subject_id, source_collection_id);

-- Vocabulary

CREATE TABLE vocabulary_entries (
    id BIGINT GENERATED BY DEFAULT AS IDENTITY PRIMARY KEY,
    source_subject_id BIGINT NOT NULL,
    material_type TEXT NOT NULL DEFAULT 'vocabulary'
        CHECK (material_type = 'vocabulary'),
    spelling TEXT NOT NULL,
    reading TEXT NOT NULL,
    parts_of_speech TEXT[] NOT NULL DEFAULT '{}'::TEXT[],
    is_primary_reading_in_source BOOLEAN,
    mapping_method TEXT NOT NULL,
    has_direct_object_evidence BOOLEAN,

    FOREIGN KEY (source_subject_id, material_type)
        REFERENCES source_subjects(id, material_type)
        ON DELETE RESTRICT
);

CREATE INDEX vocabulary_entries_subject
    ON vocabulary_entries(source_subject_id, material_type);

-- Different senses may share the same spelling and reading.
CREATE INDEX vocabulary_entries_form
    ON vocabulary_entries(spelling, reading);

CREATE TABLE vocabulary_translations (
    id BIGINT GENERATED BY DEFAULT AS IDENTITY PRIMARY KEY,
    vocabulary_entry_id BIGINT NOT NULL
        REFERENCES vocabulary_entries(id) ON DELETE CASCADE,
    language_code TEXT NOT NULL,
    text TEXT NOT NULL,
    display_order INTEGER NOT NULL,
    is_primary_in_source BOOLEAN,
    origin TEXT NOT NULL,

    UNIQUE (vocabulary_entry_id, language_code, display_order)
);

CREATE TABLE vocabulary_examples (
    id BIGINT GENERATED BY DEFAULT AS IDENTITY PRIMARY KEY,
    vocabulary_entry_id BIGINT NOT NULL
        REFERENCES vocabulary_entries(id) ON DELETE CASCADE,
    sentence TEXT NOT NULL,
    translation TEXT,
    translation_language_code TEXT,
    origin TEXT NOT NULL
);

CREATE INDEX vocabulary_examples_entry
    ON vocabulary_examples(vocabulary_entry_id);

-- Grammar

CREATE TABLE grammar_entries (
    source_subject_id BIGINT PRIMARY KEY,
    material_type TEXT NOT NULL DEFAULT 'grammar'
        CHECK (material_type = 'grammar'),
    assessment_rule_codes TEXT[] NOT NULL DEFAULT '{}'::TEXT[],

    FOREIGN KEY (source_subject_id, material_type)
        REFERENCES source_subjects(id, material_type)
        ON DELETE RESTRICT
);

CREATE TABLE grammar_descriptions (
    id BIGINT GENERATED BY DEFAULT AS IDENTITY PRIMARY KEY,
    grammar_entry_id BIGINT NOT NULL
        REFERENCES grammar_entries(source_subject_id) ON DELETE CASCADE,
    language_code TEXT NOT NULL,
    text TEXT NOT NULL,
    display_order INTEGER NOT NULL,
    origin TEXT NOT NULL,

    UNIQUE (grammar_entry_id, language_code, display_order)
);

CREATE TABLE grammar_examples (
    id BIGINT GENERATED BY DEFAULT AS IDENTITY PRIMARY KEY,
    grammar_entry_id BIGINT NOT NULL
        REFERENCES grammar_entries(source_subject_id) ON DELETE CASCADE,
    sentence TEXT NOT NULL,
    translation TEXT,
    translation_language_code TEXT,
    origin TEXT NOT NULL
);

CREATE INDEX grammar_examples_entry
    ON grammar_examples(grammar_entry_id);

-- Embeddings

CREATE TABLE embedding_profiles (
    id BIGINT GENERATED BY DEFAULT AS IDENTITY PRIMARY KEY,
    provider TEXT NOT NULL,
    model TEXT NOT NULL,
    model_revision TEXT NOT NULL,
    dimensions INTEGER NOT NULL,
    input_format_revision TEXT NOT NULL,

    UNIQUE (provider, model, model_revision, dimensions, input_format_revision)
);

CREATE TABLE vocabulary_embeddings (
    vocabulary_entry_id BIGINT NOT NULL
        REFERENCES vocabulary_entries(id) ON DELETE CASCADE,
    embedding_profile_id BIGINT NOT NULL
        REFERENCES embedding_profiles(id) ON DELETE RESTRICT,
    input_hash TEXT NOT NULL,
    vector VECTOR NOT NULL,

    PRIMARY KEY (vocabulary_entry_id, embedding_profile_id)
);

CREATE INDEX vocabulary_embeddings_profile
    ON vocabulary_embeddings(embedding_profile_id);

CREATE TABLE grammar_embeddings (
    grammar_entry_id BIGINT NOT NULL
        REFERENCES grammar_entries(source_subject_id) ON DELETE CASCADE,
    embedding_profile_id BIGINT NOT NULL
        REFERENCES embedding_profiles(id) ON DELETE RESTRICT,
    input_hash TEXT NOT NULL,
    vector VECTOR NOT NULL,

    PRIMARY KEY (grammar_entry_id, embedding_profile_id)
);

CREATE INDEX grammar_embeddings_profile
    ON grammar_embeddings(embedding_profile_id);

COMMIT;
```
