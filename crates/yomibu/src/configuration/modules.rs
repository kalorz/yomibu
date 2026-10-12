use super::components::credential_guidance;
use serde::{Deserialize, Serialize};
use std::sync::LazyLock;
use yomibu_components::{openai_story_generation, wanikani_source};
use yomibu_core::component::Component;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ModuleId {
    Knowledge,
    Sync,
    Generation,
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

pub static MODULES: LazyLock<[ModuleMetadata; 3]> = LazyLock::new(|| {
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
            id: ModuleId::Generation,
            name: "OpenAI generation",
            purpose: "Create an experimental Japanese reading",
            settings: component_settings(&openai_story_generation::COMPONENT, &[]),
            guidance: component_guidance(&openai_story_generation::COMPONENT),
            controllable: false,
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
            Self::Generation => 2,
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
