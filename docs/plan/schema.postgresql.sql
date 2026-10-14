-- Planned PostgreSQL 18+ schema, not a runtime migration. Requires pgvector.
-- Independent IDs default to UUIDv7; callers may supply IDs created elsewhere.
-- Content validation and private-collection access policies belong to the application.
-- VECTOR is dimensionless here because embedding profiles can use different dimensions.
-- Long descriptive TEXT is not a unique key: PostgreSQL B-tree keys have size limits.
-- Exact-text deduplication belongs to content validation; ordering remains unique.
-- JMdict import scope, matching and stable-ID requirements: jmdict-import-and-matching.md.
BEGIN;

CREATE EXTENSION IF NOT EXISTS vector;

CREATE TYPE material_kind AS ENUM ('vocabulary', 'grammar');

CREATE TABLE learners (
    id UUID PRIMARY KEY DEFAULT uuidv7(),
    name TEXT NOT NULL
);

CREATE TABLE collections (
    id UUID PRIMARY KEY DEFAULT uuidv7(),
    owner_learner_id UUID
        REFERENCES learners(id) ON DELETE RESTRICT,

    provider TEXT NOT NULL,
    name TEXT NOT NULL,
    is_reference BOOLEAN NOT NULL DEFAULT FALSE,

    -- Current dataset provenance; retain the source file separately. Not revision history.
    source_url TEXT,
    license TEXT,
    attribution TEXT,
    source_revision TEXT,
    source_checksum TEXT
);

CREATE INDEX collections_owner
    ON collections(owner_learner_id);

-- One upstream record can produce several resolved entries, or remain unresolved.
CREATE TABLE subjects (
    id UUID PRIMARY KEY DEFAULT uuidv7(),
    collection_id UUID NOT NULL
        REFERENCES collections(id) ON DELETE RESTRICT,
    external_id TEXT NOT NULL,

    -- Normalized kind; NULL until classified. Preserve raw provider types in source_data.
    material_type material_kind,

    source_data JSONB,

    UNIQUE (collection_id, external_id),
    UNIQUE (id, collection_id),
    UNIQUE (id, material_type)
);

-- Manual collections use the same association, without an external account or import.
CREATE TABLE learner_collections (
    id UUID PRIMARY KEY DEFAULT uuidv7(),
    collection_id UUID NOT NULL
        REFERENCES collections(id) ON DELETE RESTRICT,
    learner_id UUID NOT NULL
        REFERENCES learners(id) ON DELETE RESTRICT,
    external_account_id TEXT,

    last_import_started_at TIMESTAMPTZ,
    last_import_completed_at TIMESTAMPTZ,

    source_data JSONB,

    UNIQUE (learner_id, collection_id),
    UNIQUE (id, collection_id)
);

CREATE INDEX learner_collections_collection
    ON learner_collections(collection_id);

-- Store observations and manual declarations; derive eligibility in the application.
CREATE TABLE learner_progress (
    learner_collection_id UUID NOT NULL,
    subject_id UUID NOT NULL,
    collection_id UUID NOT NULL,

    progress_stage TEXT,

    is_marked_known BOOLEAN,
    is_hidden BOOLEAN NOT NULL DEFAULT FALSE,

    learning_started_at TIMESTAMPTZ,
    first_passed_at TIMESTAMPTZ,
    recorded_at TIMESTAMPTZ NOT NULL,

    source_data JSONB,

    PRIMARY KEY (learner_collection_id, subject_id),

    FOREIGN KEY (learner_collection_id, collection_id)
        REFERENCES learner_collections(id, collection_id)
        ON DELETE CASCADE,

    FOREIGN KEY (subject_id, collection_id)
        REFERENCES subjects(id, collection_id)
        ON DELETE RESTRICT
);

CREATE INDEX learner_progress_subject
    ON learner_progress(subject_id, collection_id);

-- Resolved vocabulary senses and grammar usages share identity, not their detail fields.
-- The application publishes each entry with exactly one matching detail row atomically.
-- Typed foreign keys below enforce the detail kind, not the existence of a detail row.
CREATE TABLE entries (
    id UUID PRIMARY KEY DEFAULT uuidv7(),
    subject_id UUID NOT NULL,
    kind material_kind NOT NULL,

    UNIQUE (id, kind),

    FOREIGN KEY (subject_id, kind)
        REFERENCES subjects(id, material_type)
        ON DELETE RESTRICT
);

CREATE INDEX entries_subject
    ON entries(subject_id, kind);

-- Accepted same-kind correspondences only; the application checks that the target
-- belongs to a reference collection. Shared matches are reused across learners;
-- learner progress stays on the original subject.
CREATE TABLE entry_references (
    entry_id UUID NOT NULL,
    reference_entry_id UUID NOT NULL,
    kind material_kind NOT NULL,

    match_method TEXT NOT NULL,

    PRIMARY KEY (entry_id, reference_entry_id),
    CHECK (entry_id <> reference_entry_id),

    FOREIGN KEY (entry_id, kind)
        REFERENCES entries(id, kind) ON DELETE CASCADE,

    FOREIGN KEY (reference_entry_id, kind)
        REFERENCES entries(id, kind) ON DELETE RESTRICT
);

-- Vocabulary

CREATE TABLE vocabulary_details (
    entry_id UUID PRIMARY KEY,
    kind material_kind GENERATED ALWAYS AS ('vocabulary'::material_kind) STORED NOT NULL,

    spelling TEXT NOT NULL,
    reading TEXT NOT NULL,
    parts_of_speech TEXT[] NOT NULL DEFAULT '{}'::TEXT[],

    is_primary_reading_in_source BOOLEAN,
    has_direct_object_evidence BOOLEAN,

    FOREIGN KEY (entry_id, kind)
        REFERENCES entries(id, kind)
        ON DELETE CASCADE
);

-- Different senses may share the same spelling and reading.
CREATE INDEX vocabulary_details_form
    ON vocabulary_details(spelling, reading);

CREATE TABLE vocabulary_translations (
    id UUID PRIMARY KEY DEFAULT uuidv7(),
    entry_id UUID NOT NULL
        REFERENCES vocabulary_details(entry_id) ON DELETE CASCADE,

    language_code TEXT NOT NULL,
    text TEXT NOT NULL,
    position INTEGER NOT NULL,

    is_primary_in_source BOOLEAN,

    origin TEXT NOT NULL,

    UNIQUE (entry_id, language_code, position)
);

CREATE TABLE vocabulary_examples (
    id UUID PRIMARY KEY DEFAULT uuidv7(),
    entry_id UUID NOT NULL
        REFERENCES vocabulary_details(entry_id) ON DELETE CASCADE,

    sentence TEXT NOT NULL,
    translation_language_code TEXT,
    translation TEXT,

    origin TEXT NOT NULL
);

CREATE INDEX vocabulary_examples_entry
    ON vocabulary_examples(entry_id);

-- Grammar

CREATE TABLE grammar_details (
    entry_id UUID PRIMARY KEY,
    kind material_kind GENERATED ALWAYS AS ('grammar'::material_kind) STORED NOT NULL,

    headword TEXT NOT NULL,
    reading TEXT,

    -- Imported explanations do not imply executable checks; bind supported rules explicitly.
    assessment_rule_codes TEXT[] NOT NULL DEFAULT '{}'::TEXT[],

    FOREIGN KEY (entry_id, kind)
        REFERENCES entries(id, kind)
        ON DELETE CASCADE
);

CREATE TABLE grammar_descriptions (
    id UUID PRIMARY KEY DEFAULT uuidv7(),
    entry_id UUID NOT NULL
        REFERENCES grammar_details(entry_id) ON DELETE CASCADE,

    language_code TEXT NOT NULL,
    kind TEXT NOT NULL, -- Description purpose; categories remain to be agreed.
    text TEXT NOT NULL,
    position INTEGER NOT NULL,

    origin TEXT NOT NULL,

    UNIQUE (entry_id, language_code, position)
);

-- Descriptive connection patterns, not executable parsing rules.
CREATE TABLE grammar_formations (
    id UUID PRIMARY KEY DEFAULT uuidv7(),
    entry_id UUID NOT NULL
        REFERENCES grammar_details(entry_id) ON DELETE CASCADE,

    pattern TEXT NOT NULL,
    position INTEGER NOT NULL,

    UNIQUE (entry_id, position),
    UNIQUE (id, entry_id)
);

CREATE TABLE grammar_examples (
    id UUID PRIMARY KEY DEFAULT uuidv7(),
    entry_id UUID NOT NULL
        REFERENCES grammar_details(entry_id) ON DELETE CASCADE,
    formation_id UUID, -- NULL for examples of the usage without a specific formation.

    sentence TEXT NOT NULL,
    translation_language_code TEXT,
    translation TEXT,

    origin TEXT NOT NULL,

    FOREIGN KEY (formation_id, entry_id)
        REFERENCES grammar_formations(id, entry_id)
        ON DELETE CASCADE
);

CREATE INDEX grammar_examples_entry
    ON grammar_examples(entry_id);

-- Embeddings

CREATE TABLE embedding_profiles (
    id UUID PRIMARY KEY DEFAULT uuidv7(),

    provider TEXT NOT NULL,
    model TEXT NOT NULL,
    model_revision TEXT NOT NULL,
    dimensions INTEGER NOT NULL,

    input_format_revision TEXT NOT NULL,

    UNIQUE (provider, model, model_revision, dimensions, input_format_revision),
    UNIQUE (id, dimensions)
);

CREATE TABLE entry_embeddings (
    entry_id UUID NOT NULL
        REFERENCES entries(id) ON DELETE CASCADE,
    embedding_profile_id UUID NOT NULL,

    dimensions INTEGER NOT NULL,
    vector VECTOR NOT NULL,

    input_hash TEXT NOT NULL,

    PRIMARY KEY (entry_id, embedding_profile_id),
    CHECK (vector_dims(vector) = dimensions),

    FOREIGN KEY (embedding_profile_id, dimensions)
        REFERENCES embedding_profiles(id, dimensions)
        ON DELETE RESTRICT
);

CREATE INDEX entry_embeddings_profile
    ON entry_embeddings(embedding_profile_id);

COMMIT;
