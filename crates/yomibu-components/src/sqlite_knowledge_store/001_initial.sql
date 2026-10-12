CREATE TABLE learners (
    learner_id TEXT PRIMARY KEY NOT NULL
);
CREATE TABLE source_collections (
    collection_id TEXT PRIMARY KEY NOT NULL,
    source_kind TEXT NOT NULL
);
CREATE TABLE source_subjects (
    subject_id INTEGER PRIMARY KEY,
    collection_id TEXT NOT NULL,
    external_id TEXT NOT NULL,
    UNIQUE (collection_id, external_id),
    UNIQUE (subject_id, collection_id),
    FOREIGN KEY (collection_id) REFERENCES source_collections (collection_id) ON DELETE RESTRICT
);
CREATE TABLE source_learner_profiles (
    profile_id TEXT PRIMARY KEY NOT NULL,
    learner_id TEXT NOT NULL,
    collection_id TEXT NOT NULL,
    provider_account_id TEXT,
    access_expires_at TEXT,
    UNIQUE (profile_id, collection_id),
    UNIQUE (learner_id, collection_id, provider_account_id),
    FOREIGN KEY (learner_id) REFERENCES learners (learner_id) ON DELETE RESTRICT,
    FOREIGN KEY (collection_id) REFERENCES source_collections (collection_id) ON DELETE RESTRICT
);
CREATE TABLE source_profile_subjects (
    profile_id TEXT NOT NULL,
    subject_id INTEGER NOT NULL,
    collection_id TEXT NOT NULL,
    position INTEGER NOT NULL,
    content_kind TEXT NOT NULL,
    PRIMARY KEY (profile_id, subject_id),
    UNIQUE (profile_id, position),
    FOREIGN KEY (profile_id, collection_id)
        REFERENCES source_learner_profiles (profile_id, collection_id) ON DELETE CASCADE,
    FOREIGN KEY (subject_id, collection_id)
        REFERENCES source_subjects (subject_id, collection_id) ON DELETE RESTRICT
);
CREATE TABLE source_vocabulary (
    subject_id INTEGER PRIMARY KEY,
    collection_id TEXT NOT NULL,
    word_id TEXT NOT NULL,
    written_form TEXT NOT NULL,
    direct_object INTEGER,
    UNIQUE (collection_id, word_id),
    FOREIGN KEY (subject_id, collection_id)
        REFERENCES source_subjects (subject_id, collection_id) ON DELETE CASCADE
);
CREATE TABLE source_vocabulary_readings (
    subject_id INTEGER NOT NULL,
    position INTEGER NOT NULL,
    reading TEXT NOT NULL,
    PRIMARY KEY (subject_id, position),
    FOREIGN KEY (subject_id) REFERENCES source_vocabulary (subject_id) ON DELETE CASCADE
);
CREATE TABLE source_vocabulary_meanings (
    subject_id INTEGER NOT NULL,
    position INTEGER NOT NULL,
    meaning TEXT NOT NULL,
    PRIMARY KEY (subject_id, position),
    FOREIGN KEY (subject_id) REFERENCES source_vocabulary (subject_id) ON DELETE CASCADE
);
CREATE TABLE source_learner_progress (
    profile_id TEXT NOT NULL,
    subject_id INTEGER NOT NULL,
    is_marked_known INTEGER,
    learning_started_at TEXT,
    first_passed_at TEXT,
    is_hidden INTEGER NOT NULL,
    PRIMARY KEY (profile_id, subject_id),
    FOREIGN KEY (profile_id, subject_id)
        REFERENCES source_profile_subjects (profile_id, subject_id) ON DELETE CASCADE
);
CREATE INDEX source_learner_profiles_by_collection
    ON source_learner_profiles (collection_id);
CREATE INDEX source_profile_subjects_by_subject
    ON source_profile_subjects (subject_id, collection_id);
