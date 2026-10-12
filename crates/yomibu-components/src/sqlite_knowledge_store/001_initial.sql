CREATE TABLE learners (
    id TEXT PRIMARY KEY NOT NULL
);
CREATE TABLE source_collections (
    id TEXT PRIMARY KEY NOT NULL,
    provider TEXT NOT NULL
);
CREATE TABLE source_subjects (
    id INTEGER PRIMARY KEY,
    source_collection_id TEXT NOT NULL,
    external_id TEXT NOT NULL,
    UNIQUE (source_collection_id, external_id),
    UNIQUE (id, source_collection_id),
    FOREIGN KEY (source_collection_id) REFERENCES source_collections (id) ON DELETE RESTRICT
);
CREATE TABLE source_learner_profiles (
    id TEXT PRIMARY KEY NOT NULL,
    learner_id TEXT NOT NULL,
    source_collection_id TEXT NOT NULL,
    external_account_id TEXT,
    access_expires_at TEXT,
    UNIQUE (id, source_collection_id),
    UNIQUE (learner_id, source_collection_id, external_account_id),
    FOREIGN KEY (learner_id) REFERENCES learners (id) ON DELETE RESTRICT,
    FOREIGN KEY (source_collection_id) REFERENCES source_collections (id) ON DELETE RESTRICT
);
CREATE TABLE source_profile_subjects (
    source_learner_profile_id TEXT NOT NULL,
    source_subject_id INTEGER NOT NULL,
    source_collection_id TEXT NOT NULL,
    position INTEGER NOT NULL,
    content_kind TEXT NOT NULL,
    PRIMARY KEY (source_learner_profile_id, source_subject_id),
    UNIQUE (source_learner_profile_id, position),
    FOREIGN KEY (source_learner_profile_id, source_collection_id)
        REFERENCES source_learner_profiles (id, source_collection_id) ON DELETE CASCADE,
    FOREIGN KEY (source_subject_id, source_collection_id)
        REFERENCES source_subjects (id, source_collection_id) ON DELETE RESTRICT
);
CREATE TABLE source_vocabulary (
    source_subject_id INTEGER PRIMARY KEY,
    source_collection_id TEXT NOT NULL,
    word_id TEXT NOT NULL,
    spelling TEXT NOT NULL,
    has_direct_object_evidence INTEGER,
    UNIQUE (source_collection_id, word_id),
    FOREIGN KEY (source_subject_id, source_collection_id)
        REFERENCES source_subjects (id, source_collection_id) ON DELETE CASCADE
);
CREATE TABLE source_vocabulary_readings (
    source_subject_id INTEGER NOT NULL,
    position INTEGER NOT NULL,
    reading TEXT NOT NULL,
    PRIMARY KEY (source_subject_id, position),
    FOREIGN KEY (source_subject_id) REFERENCES source_vocabulary (source_subject_id) ON DELETE CASCADE
);
CREATE TABLE source_vocabulary_meanings (
    source_subject_id INTEGER NOT NULL,
    position INTEGER NOT NULL,
    meaning TEXT NOT NULL,
    PRIMARY KEY (source_subject_id, position),
    FOREIGN KEY (source_subject_id) REFERENCES source_vocabulary (source_subject_id) ON DELETE CASCADE
);
CREATE TABLE source_learner_progress (
    source_learner_profile_id TEXT NOT NULL,
    source_subject_id INTEGER NOT NULL,
    is_marked_known INTEGER,
    learning_started_at TEXT,
    first_passed_at TEXT,
    is_hidden INTEGER NOT NULL,
    PRIMARY KEY (source_learner_profile_id, source_subject_id),
    FOREIGN KEY (source_learner_profile_id, source_subject_id)
        REFERENCES source_profile_subjects (source_learner_profile_id, source_subject_id) ON DELETE CASCADE
);
CREATE INDEX source_learner_profiles_collection
    ON source_learner_profiles (source_collection_id);
CREATE INDEX source_profile_subjects_by_subject
    ON source_profile_subjects (source_subject_id, source_collection_id);
