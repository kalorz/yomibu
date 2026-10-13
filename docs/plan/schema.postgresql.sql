-- PostgreSQL 18+ reference schema. Requires the pgvector extension on the server.
-- Independent IDs default to UUIDv7; callers may supply IDs created elsewhere.
-- Content validation and private-collection access policies belong to the application.
-- VECTOR is dimensionless here because embedding profiles can use different dimensions.
-- Long descriptive TEXT is not a unique key: PostgreSQL B-tree keys have size limits.
-- Exact-text deduplication belongs to content validation; ordering remains unique.
BEGIN;

CREATE EXTENSION IF NOT EXISTS vector;

CREATE TABLE learners (
    id UUID PRIMARY KEY DEFAULT uuidv7(),
    name TEXT NOT NULL
);

CREATE TABLE source_collections (
    id UUID PRIMARY KEY DEFAULT uuidv7(),
    owner_learner_id UUID
        REFERENCES learners(id) ON DELETE RESTRICT,

    provider TEXT NOT NULL,
    name TEXT NOT NULL
);

CREATE INDEX source_collections_owner
    ON source_collections(owner_learner_id);

CREATE TABLE source_subjects (
    id UUID PRIMARY KEY DEFAULT uuidv7(),
    source_collection_id UUID NOT NULL
        REFERENCES source_collections(id) ON DELETE RESTRICT,
    external_id TEXT NOT NULL,

    material_type TEXT, -- NULL when the source subject's type is unknown.

    source_data JSONB,

    UNIQUE (source_collection_id, external_id),
    UNIQUE (id, source_collection_id),
    UNIQUE (id, material_type)
);

CREATE TABLE source_learner_profiles (
    id UUID PRIMARY KEY DEFAULT uuidv7(),
    source_collection_id UUID NOT NULL
        REFERENCES source_collections(id) ON DELETE RESTRICT,
    learner_id UUID NOT NULL
        REFERENCES learners(id) ON DELETE RESTRICT,
    external_account_id TEXT,

    last_import_started_at TIMESTAMPTZ,
    last_import_completed_at TIMESTAMPTZ,

    source_data JSONB,

    UNIQUE (learner_id, source_collection_id),
    UNIQUE (id, source_collection_id)
);

CREATE INDEX source_learner_profiles_collection
    ON source_learner_profiles(source_collection_id);

CREATE TABLE source_learner_progress (
    source_learner_profile_id UUID NOT NULL,
    source_subject_id UUID NOT NULL,
    source_collection_id UUID NOT NULL,

    progress_stage TEXT,

    is_marked_known BOOLEAN,
    is_hidden BOOLEAN NOT NULL DEFAULT FALSE,

    learning_started_at TIMESTAMPTZ,
    first_passed_at TIMESTAMPTZ,
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
    id UUID PRIMARY KEY DEFAULT uuidv7(),
    source_subject_id UUID NOT NULL,
    material_type TEXT GENERATED ALWAYS AS ('vocabulary'::TEXT) STORED NOT NULL,

    spelling TEXT NOT NULL,
    reading TEXT NOT NULL,
    parts_of_speech TEXT[] NOT NULL DEFAULT '{}'::TEXT[],

    is_primary_reading_in_source BOOLEAN,
    has_direct_object_evidence BOOLEAN,

    mapping_method TEXT NOT NULL,

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
    id UUID PRIMARY KEY DEFAULT uuidv7(),
    vocabulary_entry_id UUID NOT NULL
        REFERENCES vocabulary_entries(id) ON DELETE CASCADE,

    language_code TEXT NOT NULL,
    text TEXT NOT NULL,
    position INTEGER NOT NULL,

    is_primary_in_source BOOLEAN,

    origin TEXT NOT NULL,

    UNIQUE (vocabulary_entry_id, language_code, position)
);

CREATE TABLE vocabulary_examples (
    id UUID PRIMARY KEY DEFAULT uuidv7(),
    vocabulary_entry_id UUID NOT NULL
        REFERENCES vocabulary_entries(id) ON DELETE CASCADE,

    sentence TEXT NOT NULL,
    translation_language_code TEXT,
    translation TEXT,

    origin TEXT NOT NULL
);

CREATE INDEX vocabulary_examples_entry
    ON vocabulary_examples(vocabulary_entry_id);

-- Grammar

CREATE TABLE grammar_entries (
    id UUID PRIMARY KEY,
    material_type TEXT GENERATED ALWAYS AS ('grammar'::TEXT) STORED NOT NULL,

    assessment_rule_codes TEXT[] NOT NULL DEFAULT '{}'::TEXT[],

    FOREIGN KEY (id, material_type)
        REFERENCES source_subjects(id, material_type)
        ON DELETE RESTRICT
);

CREATE TABLE grammar_descriptions (
    id UUID PRIMARY KEY DEFAULT uuidv7(),
    grammar_entry_id UUID NOT NULL
        REFERENCES grammar_entries(id) ON DELETE CASCADE,

    language_code TEXT NOT NULL,
    text TEXT NOT NULL,
    position INTEGER NOT NULL,

    origin TEXT NOT NULL,

    UNIQUE (grammar_entry_id, language_code, position)
);

CREATE TABLE grammar_examples (
    id UUID PRIMARY KEY DEFAULT uuidv7(),
    grammar_entry_id UUID NOT NULL
        REFERENCES grammar_entries(id) ON DELETE CASCADE,

    sentence TEXT NOT NULL,
    translation_language_code TEXT,
    translation TEXT,

    origin TEXT NOT NULL
);

CREATE INDEX grammar_examples_entry
    ON grammar_examples(grammar_entry_id);

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

CREATE TABLE vocabulary_embeddings (
    vocabulary_entry_id UUID NOT NULL
        REFERENCES vocabulary_entries(id) ON DELETE CASCADE,
    embedding_profile_id UUID NOT NULL,

    dimensions INTEGER NOT NULL,
    vector VECTOR NOT NULL,

    input_hash TEXT NOT NULL,

    PRIMARY KEY (vocabulary_entry_id, embedding_profile_id),
    CHECK (vector_dims(vector) = dimensions),

    FOREIGN KEY (embedding_profile_id, dimensions)
        REFERENCES embedding_profiles(id, dimensions)
        ON DELETE RESTRICT
);

CREATE INDEX vocabulary_embeddings_profile
    ON vocabulary_embeddings(embedding_profile_id);

CREATE TABLE grammar_embeddings (
    grammar_entry_id UUID NOT NULL
        REFERENCES grammar_entries(id) ON DELETE CASCADE,
    embedding_profile_id UUID NOT NULL,

    dimensions INTEGER NOT NULL,
    vector VECTOR NOT NULL,

    input_hash TEXT NOT NULL,

    PRIMARY KEY (grammar_entry_id, embedding_profile_id),
    CHECK (vector_dims(vector) = dimensions),

    FOREIGN KEY (embedding_profile_id, dimensions)
        REFERENCES embedding_profiles(id, dimensions)
        ON DELETE RESTRICT
);

CREATE INDEX grammar_embeddings_profile
    ON grammar_embeddings(embedding_profile_id);

COMMIT;
