use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ModuleId {
    Knowledge,
    Sync,
    Embeddings,
    Generation,
    Assessment,
}

#[derive(Debug, Serialize)]
pub struct ModuleMetadata {
    pub id: ModuleId,
    pub name: &'static str,
    pub purpose: &'static str,
    pub settings: &'static [&'static str],
    pub guidance: &'static str,
    pub controllable: bool,
    pub default_enabled: bool,
}

pub static MODULES: &[ModuleMetadata] = &[
    ModuleMetadata {
        id: ModuleId::Knowledge,
        name: "Learner vocabulary",
        purpose: "Select words you have studied",
        settings: &["--inventory", "--enable sync"],
        guidance: "Supply a manual inventory or usable cache; enable sync with a WaniKani key to refresh it.",
        controllable: false,
        default_enabled: true,
    },
    ModuleMetadata {
        id: ModuleId::Sync,
        name: "WaniKani sync",
        purpose: "Refresh source progress",
        settings: &[
            "--wanikani-source-api-key",
            "YOMIBU_WANIKANI_SOURCE_API_KEY",
        ],
        guidance: "Use a WaniKani API key with read access and no write permissions. On macOS, run yomibu auth wanikani.",
        controllable: true,
        default_enabled: true,
    },
    ModuleMetadata {
        id: ModuleId::Embeddings,
        name: "Embeddings",
        purpose: "Improve topic vocabulary selection",
        settings: &[
            "--embedding-provider",
            "--http-embeddings-model",
            "--http-embeddings-revision",
            "--http-embeddings-dimensions",
        ],
        guidance: "Enable embeddings and configure an encoder. Hosted requests also require --allow-embedding-call.",
        controllable: true,
        default_enabled: false,
    },
    ModuleMetadata {
        id: ModuleId::Generation,
        name: "OpenAI generation",
        purpose: "Create an experimental Japanese reading",
        settings: &[
            "--openai-story-generation-api-key",
            "YOMIBU_OPENAI_STORY_GENERATION_API_KEY",
        ],
        guidance: "Use an OpenAI key with response creation (api.responses.write) and access to the selected model. On macOS, run yomibu auth openai.",
        controllable: false,
        default_enabled: true,
    },
    ModuleMetadata {
        id: ModuleId::Assessment,
        name: "Sudachi assessment",
        purpose: "Check available vocabulary evidence",
        settings: &["--dictionary-dir"],
        guidance: "Import a dictionary with yomibu dictionary import. No dictionary is downloaded automatically.",
        controllable: true,
        default_enabled: true,
    },
];

impl ModuleId {
    pub fn metadata(self) -> &'static ModuleMetadata {
        &MODULES[match self {
            Self::Knowledge => 0,
            Self::Sync => 1,
            Self::Embeddings => 2,
            Self::Generation => 3,
            Self::Assessment => 4,
        }]
    }
}

#[derive(Debug, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum ModuleState {
    Disabled,
    NotConfigured,
    Available,
    Skipped { reason: String },
    Unavailable { error: String },
}

#[derive(Debug, Serialize)]
pub struct ModuleReport {
    pub metadata: &'static ModuleMetadata,
    pub state: ModuleState,
    pub required: bool,
}
