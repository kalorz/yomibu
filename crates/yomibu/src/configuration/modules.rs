use super::components::credential_guidance;
use serde::{Deserialize, Serialize};
use std::sync::LazyLock;
use yomibu_components::{http_embeddings, openai_story_generation, wanikani_source};
use yomibu_core::component::Component;

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
    pub settings: Vec<String>,
    pub guidance: String,
    pub controllable: bool,
    pub default_enabled: bool,
}

pub static MODULES: LazyLock<[ModuleMetadata; 5]> = LazyLock::new(|| {
    [
        ModuleMetadata {
            id: ModuleId::Knowledge,
            name: "Learner vocabulary",
            purpose: "Select words you have studied",
            settings: vec!["--inventory".into(), "--enable sync".into()],
            guidance: "Supply --inventory PATH or a usable cache; use --enable sync with a WaniKani key to refresh it.".into(),
            controllable: false,
            default_enabled: true,
        },
        ModuleMetadata {
            id: ModuleId::Sync,
            name: "WaniKani sync",
            purpose: "Refresh source progress",
            settings: component_settings(&wanikani_source::COMPONENT, &[]),
            guidance: component_guidance(&wanikani_source::COMPONENT),
            controllable: true,
            default_enabled: true,
        },
        ModuleMetadata {
            id: ModuleId::Embeddings,
            name: "Embeddings",
            purpose: "Improve topic vocabulary selection",
            settings: component_settings(&http_embeddings::COMPONENT, &["--embedding-provider"]),
            guidance: format!(
                "Enable embeddings and configure an encoder. Hosted requests require --allow-embedding-call. {}",
                component_guidance(&http_embeddings::COMPONENT)
            ),
            controllable: true,
            default_enabled: false,
        },
        ModuleMetadata {
            id: ModuleId::Generation,
            name: "OpenAI generation",
            purpose: "Create an experimental Japanese reading",
            settings: component_settings(&openai_story_generation::COMPONENT, &[]),
            guidance: component_guidance(&openai_story_generation::COMPONENT),
            controllable: false,
            default_enabled: true,
        },
        ModuleMetadata {
            id: ModuleId::Assessment,
            name: "Sudachi assessment",
            purpose: "Check available vocabulary evidence",
            settings: vec!["--dictionary-dir".into()],
            guidance: "Import a dictionary with yomibu dictionary import; use --dictionary-dir PATH for a custom location. No dictionary is downloaded automatically.".into(),
            controllable: true,
            default_enabled: true,
        },
    ]
});

fn component_settings(component: &Component, application_flags: &[&str]) -> Vec<String> {
    let mut settings: Vec<_> = application_flags.iter().map(|s| (*s).to_owned()).collect();
    for setting in component.settings {
        settings.push(format!("--{}", setting.name().cli()));
        if setting.secret().is_some() {
            settings.push(setting.name().environment());
        }
    }
    settings
}

fn component_guidance(component: &Component) -> String {
    component
        .settings
        .iter()
        .filter_map(|s| s.secret())
        .map(credential_guidance)
        .collect::<Vec<_>>()
        .join(" ")
}

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
