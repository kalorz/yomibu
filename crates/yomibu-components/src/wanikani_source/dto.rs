use chrono::{DateTime, Utc};
use serde::Deserialize;
use yomibu_core::domain::source::{Learner, Subscription};

#[derive(Deserialize)]
pub(super) struct User {
    pub object: String,
    data_updated_at: DateTime<Utc>,
    data: UserData,
}

#[derive(Deserialize)]
struct UserData {
    id: String,
    username: String,
    level: u32,
    started_at: DateTime<Utc>,
    #[serde(deserialize_with = "Deserialize::deserialize")]
    current_vacation_started_at: Option<DateTime<Utc>>,
    subscription: SubscriptionData,
}

#[derive(Deserialize)]
struct SubscriptionData {
    active: bool,
    #[serde(rename = "type")]
    kind: String,
    max_level_granted: u32,
    #[serde(deserialize_with = "Deserialize::deserialize")]
    period_ends_at: Option<DateTime<Utc>>,
}

impl From<User> for Learner {
    fn from(user: User) -> Self {
        let data = user.data;
        Self {
            id: data.id,
            username: data.username,
            level: data.level,
            updated_at: user.data_updated_at,
            started_at: data.started_at,
            current_vacation_started_at: data.current_vacation_started_at,
            subscription: Subscription {
                active: data.subscription.active,
                kind: data.subscription.kind,
                max_level_granted: data.subscription.max_level_granted,
                period_ends_at: data.subscription.period_ends_at,
            },
        }
    }
}

#[derive(Deserialize)]
pub(super) struct Collection<T> {
    pub pages: Pages,
    pub object: String,
    pub data: Vec<T>,
}

use yomibu_core::domain::{
    source as domain,
    source::{Assignment, LexicalContent, ReviewStatistic, Subject, SubjectKind},
};

#[derive(Deserialize)]
pub(super) struct Resource<T> {
    pub id: u64,
    pub object: String,
    pub data_updated_at: DateTime<Utc>,
    pub data: T,
}

#[derive(Deserialize, Clone, Copy)]
#[serde(rename_all = "snake_case")]
pub(super) enum Kind {
    Radical,
    Kanji,
    Vocabulary,
    KanaVocabulary,
}
impl Kind {
    pub fn retained(self) -> Option<SubjectKind> {
        match self {
            Self::Radical => None,
            Self::Kanji => Some(SubjectKind::Kanji),
            Self::Vocabulary => Some(SubjectKind::Vocabulary),
            Self::KanaVocabulary => Some(SubjectKind::KanaVocabulary),
        }
    }
}

#[derive(Deserialize)]
pub(super) struct AssignmentData {
    subject_id: u64,
    subject_type: Kind,
    srs_stage: u32,
    hidden: bool,
    created_at: DateTime<Utc>,
    #[serde(deserialize_with = "Deserialize::deserialize")]
    unlocked_at: Option<DateTime<Utc>>,
    #[serde(deserialize_with = "Deserialize::deserialize")]
    started_at: Option<DateTime<Utc>>,
    #[serde(deserialize_with = "Deserialize::deserialize")]
    passed_at: Option<DateTime<Utc>>,
    #[serde(deserialize_with = "Deserialize::deserialize")]
    burned_at: Option<DateTime<Utc>>,
    #[serde(deserialize_with = "Deserialize::deserialize")]
    available_at: Option<DateTime<Utc>>,
    #[serde(deserialize_with = "Deserialize::deserialize")]
    resurrected_at: Option<DateTime<Utc>>,
}
impl Resource<AssignmentData> {
    pub fn normalize(self) -> Option<Assignment> {
        let data = self.data;
        Some(Assignment {
            id: self.id,
            subject_id: data.subject_id,
            subject_kind: data.subject_type.retained()?,
            srs_stage: data.srs_stage,
            hidden: data.hidden,
            updated_at: self.data_updated_at,
            created_at: data.created_at,
            unlocked_at: data.unlocked_at,
            started_at: data.started_at,
            passed_at: data.passed_at,
            burned_at: data.burned_at,
            available_at: data.available_at,
            resurrected_at: data.resurrected_at,
        })
    }
}

#[derive(Deserialize)]
pub(super) struct ReviewStatisticData {
    subject_id: u64,
    subject_type: Kind,
    hidden: bool,
    created_at: DateTime<Utc>,
    reading_correct: u64,
    reading_incorrect: u64,
    reading_current_streak: u64,
    reading_max_streak: u64,
    meaning_correct: u64,
    meaning_incorrect: u64,
    meaning_current_streak: u64,
    meaning_max_streak: u64,
    percentage_correct: u32,
}
impl Resource<ReviewStatisticData> {
    pub fn normalize(self) -> Option<ReviewStatistic> {
        let data = self.data;
        Some(ReviewStatistic {
            id: self.id,
            subject_id: data.subject_id,
            subject_kind: data.subject_type.retained()?,
            hidden: data.hidden,
            updated_at: self.data_updated_at,
            created_at: data.created_at,
            reading_correct: data.reading_correct,
            reading_incorrect: data.reading_incorrect,
            reading_current_streak: data.reading_current_streak,
            reading_max_streak: data.reading_max_streak,
            meaning_correct: data.meaning_correct,
            meaning_incorrect: data.meaning_incorrect,
            meaning_current_streak: data.meaning_current_streak,
            meaning_max_streak: data.meaning_max_streak,
            percentage_correct: data.percentage_correct,
        })
    }
}

#[derive(Deserialize)]
pub(super) struct SubjectResource {
    id: u64,
    data_updated_at: DateTime<Utc>,
    #[serde(flatten)]
    content: SubjectContent,
}

#[derive(Deserialize)]
#[serde(tag = "object", content = "data", rename_all = "snake_case")]
enum SubjectContent {
    Kanji {
        #[serde(flatten)]
        common: SubjectData,
        readings: Vec<KanjiReading>,
    },
    Vocabulary {
        #[serde(flatten)]
        common: SubjectData,
        readings: Vec<domain::Reading>,
        parts_of_speech: Vec<String>,
        context_sentences: Vec<ContextSentence>,
    },
    KanaVocabulary {
        #[serde(flatten)]
        common: SubjectData,
        parts_of_speech: Vec<String>,
        context_sentences: Vec<ContextSentence>,
    },
}

#[derive(Deserialize)]
struct SubjectData {
    level: u32,
    characters: String,
    meanings: Vec<domain::Meaning>,
    created_at: DateTime<Utc>,
    #[serde(deserialize_with = "Deserialize::deserialize")]
    hidden_at: Option<DateTime<Utc>>,
    spaced_repetition_system_id: u64,
}

#[derive(Deserialize)]
struct KanjiReading {
    reading: String,
    primary: bool,
    accepted_answer: bool,
    #[serde(rename = "type")]
    kind: String,
}

#[derive(Deserialize)]
struct ContextSentence {
    ja: String,
    en: String,
}
impl From<ContextSentence> for domain::ContextSentence {
    fn from(sentence: ContextSentence) -> Self {
        Self {
            japanese: sentence.ja,
            english: sentence.en,
        }
    }
}

impl From<SubjectResource> for Subject {
    fn from(resource: SubjectResource) -> Self {
        let (common, lexical) = match resource.content {
            SubjectContent::Kanji { common, readings } => (
                common,
                LexicalContent::Kanji {
                    readings: readings
                        .into_iter()
                        .map(|reading| domain::KanjiReading {
                            reading: reading.reading,
                            primary: reading.primary,
                            accepted_answer: reading.accepted_answer,
                            kind: reading.kind,
                        })
                        .collect(),
                },
            ),
            SubjectContent::Vocabulary {
                common,
                readings,
                parts_of_speech,
                context_sentences,
            } => (
                common,
                LexicalContent::Vocabulary {
                    readings,
                    parts_of_speech,
                    context_sentences: context_sentences.into_iter().map(Into::into).collect(),
                },
            ),
            SubjectContent::KanaVocabulary {
                common,
                parts_of_speech,
                context_sentences,
            } => (
                common,
                LexicalContent::KanaVocabulary {
                    parts_of_speech,
                    context_sentences: context_sentences.into_iter().map(Into::into).collect(),
                },
            ),
        };
        Self {
            id: resource.id,
            updated_at: resource.data_updated_at,
            level: common.level,
            characters: common.characters,
            meanings: common.meanings,
            created_at: common.created_at,
            hidden_at: common.hidden_at,
            srs_system_id: common.spaced_repetition_system_id,
            lexical,
        }
    }
}

#[derive(Deserialize)]
pub(super) struct Pages {
    // An absent field cannot silently truncate a refresh; explicit null terminates it.
    #[serde(deserialize_with = "Deserialize::deserialize")]
    pub next_url: Option<String>,
}
