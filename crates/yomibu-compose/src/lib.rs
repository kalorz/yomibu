//! The composition root: builds infrastructure once and components per composition.

use std::error::Error;
use std::fmt;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;

use serde::Deserialize;
use serde::de::DeserializeOwned;
use yomibu_adapters::echo::generator::{EchoGenerator, EchoOptions};
use yomibu_adapters::text_file::inventory::TextFileInventory;
use yomibu_app::write_story::WriteStory;
use yomibu_core::ports::capabilities::generator::Generator;
use yomibu_core::ports::infrastructure::inventory_source::InventorySource;

/// The configuration file as a value: infrastructure, then components per operation.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Configuration {
    pub infrastructure: Infrastructure,
    pub story: StoryComponents,
}

impl FromStr for Configuration {
    type Err = ConfigurationError;

    fn from_str(text: &str) -> Result<Configuration, ConfigurationError> {
        toml::from_str(text).map_err(|error| ConfigurationError {
            reason: toml_reason(&error),
        })
    }
}

/// The toml crate ends its messages with a newline, which would leave a blank line after ours.
fn toml_reason(error: &toml::de::Error) -> String {
    error.to_string().trim_end().to_string()
}

#[derive(Debug)]
pub struct ConfigurationError {
    reason: String,
}

impl fmt::Display for ConfigurationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid configuration: {}", self.reason)
    }
}

impl Error for ConfigurationError {}

/// Where the infrastructure is; built once per process.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Infrastructure {
    inventory: InventorySourceTable,
}

/// The components the `WriteStory` operation uses, in order per capability.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StoryComponents {
    #[serde(default)]
    generator: Vec<ComponentTable>,
}

/// An `[infrastructure.*]` table: the backend's name and whatever options follow.
#[derive(Deserialize)]
struct InventorySourceTable {
    backend: String,
    #[serde(flatten)]
    options: toml::Table,
}

/// A `[[<operation>.<capability>]]` table: the component's name and whatever options follow.
#[derive(Deserialize)]
struct ComponentTable {
    component: String,
    #[serde(flatten)]
    options: toml::Table,
}

const TEXT_FILE: &str = "text_file";

/// The names compose can build, for help and error messages.
pub const KNOWN_INVENTORY_BACKENDS: &[&str] = &[TEXT_FILE];
pub const KNOWN_GENERATORS: &[&str] = &[EchoGenerator::NAME];

fn typed_options<Options: DeserializeOwned>(
    name: &str,
    options: &toml::Table,
) -> Result<Options, BuildError> {
    options
        .clone()
        .try_into()
        .map_err(|error: toml::de::Error| BuildError::InvalidOptions {
            name: name.to_string(),
            reason: toml_reason(&error),
        })
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TextFileOptions {
    path: PathBuf,
}

pub struct CompositionRoot {
    inventory_source: Arc<dyn InventorySource>,
}

impl CompositionRoot {
    pub fn new(infrastructure: &Infrastructure) -> Result<CompositionRoot, BuildError> {
        let table = &infrastructure.inventory;
        let inventory_source: Arc<dyn InventorySource> = match table.backend.as_str() {
            TEXT_FILE => {
                let options: TextFileOptions = typed_options(&table.backend, &table.options)?;
                Arc::new(TextFileInventory::new(options.path))
            }
            _ => {
                return Err(BuildError::Unknown {
                    kind: "inventory backend",
                    name: table.backend.clone(),
                    known: KNOWN_INVENTORY_BACKENDS,
                });
            }
        };
        Ok(CompositionRoot { inventory_source })
    }

    pub fn build_write_story(
        &self,
        components: &StoryComponents,
    ) -> Result<WriteStory, BuildError> {
        let [table] = components.generator.as_slice() else {
            return Err(BuildError::GeneratorCount {
                found: components.generator.len(),
            });
        };
        let generator: Box<dyn Generator> = match table.component.as_str() {
            EchoGenerator::NAME => {
                let options: EchoOptions = typed_options(&table.component, &table.options)?;
                Box::new(EchoGenerator::new(options))
            }
            _ => {
                return Err(BuildError::Unknown {
                    kind: "generator",
                    name: table.component.clone(),
                    known: KNOWN_GENERATORS,
                });
            }
        };
        Ok(WriteStory::new(
            Arc::clone(&self.inventory_source),
            generator,
        ))
    }
}

#[derive(Debug)]
pub enum BuildError {
    /// `kind` is the backend or capability the name was looked up in.
    Unknown {
        kind: &'static str,
        name: String,
        known: &'static [&'static str],
    },
    InvalidOptions {
        name: String,
        reason: String,
    },
    GeneratorCount {
        found: usize,
    },
}

impl fmt::Display for BuildError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BuildError::Unknown { kind, name, known } => {
                write!(f, "unknown {kind} '{name}'; known: {}", known.join(", "))
            }
            BuildError::InvalidOptions { name, reason } => {
                write!(f, "invalid options for '{name}': {reason}")
            }
            BuildError::GeneratorCount { found } => {
                write!(
                    f,
                    "the story composition needs exactly one generator, found {found}"
                )
            }
        }
    }
}

impl Error for BuildError {}
