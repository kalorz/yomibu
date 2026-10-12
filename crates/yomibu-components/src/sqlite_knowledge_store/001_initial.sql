CREATE TABLE learners (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL
);
CREATE TABLE source_collections (
    id INTEGER PRIMARY KEY,
    provider TEXT NOT NULL,
    name TEXT NOT NULL,
    owner_learner_id INTEGER REFERENCES learners (id) ON DELETE RESTRICT
);
CREATE TABLE source_subjects (
    id INTEGER PRIMARY KEY,
    source_collection_id INTEGER NOT NULL,
    external_id TEXT NOT NULL,
    material_type TEXT,
    source_data TEXT,
    UNIQUE (source_collection_id, external_id),
    UNIQUE (id, source_collection_id),
    UNIQUE (id, material_type),
    FOREIGN KEY (source_collection_id) REFERENCES source_collections (id) ON DELETE RESTRICT
);
CREATE TABLE source_learner_profiles (
    id INTEGER PRIMARY KEY,
    learner_id INTEGER NOT NULL,
    source_collection_id INTEGER NOT NULL,
    external_account_id TEXT,
    access_expires_at TEXT,
    UNIQUE (id, source_collection_id),
    UNIQUE (learner_id, source_collection_id),
    FOREIGN KEY (learner_id) REFERENCES learners (id) ON DELETE RESTRICT,
    FOREIGN KEY (source_collection_id) REFERENCES source_collections (id) ON DELETE RESTRICT
);
CREATE TABLE source_profile_subjects (
    source_learner_profile_id INTEGER NOT NULL,
    source_subject_id INTEGER NOT NULL,
    source_collection_id INTEGER NOT NULL,
    position INTEGER NOT NULL,
    content_kind TEXT NOT NULL,
    PRIMARY KEY (source_learner_profile_id, source_subject_id),
    UNIQUE (source_learner_profile_id, position),
    FOREIGN KEY (source_learner_profile_id, source_collection_id)
        REFERENCES source_learner_profiles (id, source_collection_id) ON DELETE CASCADE,
    FOREIGN KEY (source_subject_id, source_collection_id)
        REFERENCES source_subjects (id, source_collection_id) ON DELETE RESTRICT
);
CREATE TABLE vocabulary_entries (
    id INTEGER PRIMARY KEY,
    source_subject_id INTEGER NOT NULL,
    material_type TEXT GENERATED ALWAYS AS ('vocabulary') STORED NOT NULL,
    spelling TEXT NOT NULL,
    reading TEXT NOT NULL,
    is_primary_reading_in_source INTEGER,
    mapping_method TEXT NOT NULL,
    has_direct_object_evidence INTEGER,
    FOREIGN KEY (source_subject_id, material_type)
        REFERENCES source_subjects (id, material_type) ON DELETE RESTRICT
);
CREATE TABLE vocabulary_entry_parts_of_speech (
    vocabulary_entry_id INTEGER NOT NULL REFERENCES vocabulary_entries (id) ON DELETE CASCADE,
    position INTEGER NOT NULL,
    part_of_speech TEXT NOT NULL,
    PRIMARY KEY (vocabulary_entry_id, position)
);
CREATE TABLE vocabulary_translations (
    id INTEGER PRIMARY KEY,
    vocabulary_entry_id INTEGER NOT NULL REFERENCES vocabulary_entries (id) ON DELETE CASCADE,
    language_code TEXT NOT NULL,
    text TEXT NOT NULL,
    display_order INTEGER NOT NULL,
    is_primary_in_source INTEGER,
    origin TEXT NOT NULL,
    UNIQUE (vocabulary_entry_id, language_code, display_order)
);
CREATE TABLE source_learner_progress (
    source_learner_profile_id INTEGER NOT NULL,
    source_subject_id INTEGER NOT NULL,
    source_collection_id INTEGER NOT NULL,
    is_marked_known INTEGER,
    learning_started_at TEXT,
    first_passed_at TEXT,
    is_hidden INTEGER NOT NULL DEFAULT FALSE,
    PRIMARY KEY (source_learner_profile_id, source_subject_id),
    FOREIGN KEY (source_learner_profile_id, source_collection_id)
        REFERENCES source_learner_profiles (id, source_collection_id) ON DELETE CASCADE,
    FOREIGN KEY (source_subject_id, source_collection_id)
        REFERENCES source_subjects (id, source_collection_id) ON DELETE RESTRICT,
    FOREIGN KEY (source_learner_profile_id, source_subject_id)
        REFERENCES source_profile_subjects (source_learner_profile_id, source_subject_id) ON DELETE CASCADE
);
CREATE INDEX source_learner_profiles_collection
    ON source_learner_profiles (source_collection_id);
CREATE INDEX source_profile_subjects_by_subject
    ON source_profile_subjects (source_subject_id, source_collection_id);
CREATE INDEX source_collections_owner
    ON source_collections (owner_learner_id);
CREATE INDEX source_learner_progress_subject
    ON source_learner_progress (source_subject_id, source_collection_id);
CREATE INDEX vocabulary_entries_subject
    ON vocabulary_entries (source_subject_id, material_type);
CREATE INDEX vocabulary_entries_form
    ON vocabulary_entries (spelling, reading);
