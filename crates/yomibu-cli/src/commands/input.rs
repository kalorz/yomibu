//! Explicit input paths and credential lookup selected by CLI arguments.
use crate::args::{KnowledgePolicy, StoryArgs};
use anyhow::{Context, Result, bail};
use serde::de::DeserializeOwned;
use std::path::Path;
use yomibu::{
    inventory::{LearnerInventory, ManualInventory},
    knowledge::{LearnerKnowledgePolicy, WaniKaniKnowledgeRule},
};

pub(super) const MAX_STORY_REQUEST_FILE_BYTES: usize = 65536;

pub(super) fn load_inventory(args: &StoryArgs) -> Result<LearnerInventory> {
    let manual = args
        .inventory
        .as_ref()
        .map(|path| read_json::<ManualInventory>(path, "Manual inventory", 4194304))
        .transpose()?;
    let inventory = if let Some(path) = &args.wanikani_cache {
        #[derive(serde::Deserialize)]
        struct Envelope {
            schema_version: u32,
            snapshot: yomibu::domain::WaniKaniSyncData,
        }
        let source: Envelope = read_json(path, "WaniKani cache", 67108864)?;
        if source.schema_version != 1 {
            bail!("Unsupported WaniKani cache version.");
        }
        let policy = LearnerKnowledgePolicy {
            wanikani: match args.knowledge_policy {
                KnowledgePolicy::LessonStarted => WaniKaniKnowledgeRule::LessonStarted,
                KnowledgePolicy::RecordedPass => WaniKaniKnowledgeRule::RecordedPass,
            },
        };
        let inventory = LearnerInventory::from_wanikani(&source.snapshot, &policy)?;
        match manual {
            Some(manual) => inventory.with_manual(manual)?,
            None => inventory,
        }
    } else {
        LearnerInventory::from_manual(manual.context("Supply --inventory or --wanikani-cache.")?)?
    };
    Ok(inventory)
}
pub(super) fn read_json<T: DeserializeOwned>(path: &Path, label: &str, limit: usize) -> Result<T> {
    let bytes =
        yomibu::adapters::input_file::read_bounded(path, label, limit, &format!("{limit} bytes"))?;
    serde_json::from_slice(&bytes).with_context(|| format!("Invalid {label} JSON"))
}
pub(super) fn read_key(name: &str) -> Result<String> {
    std::env::var(name)
        .ok()
        .filter(|v| !v.trim().is_empty())
        .with_context(|| {
            format!("Set {name} in the environment before explicitly requesting provider work.")
        })
}
