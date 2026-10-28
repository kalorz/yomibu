use std::io::ErrorKind;
use std::path::PathBuf;

use yomibu_core::domain::inventory::Inventory;
use yomibu_core::ports::infrastructure::inventory_source::{InventorySource, InventorySourceError};

/// Reads the inventory from a file with one word per line.
pub struct TextFileInventory {
    path: PathBuf,
}

impl TextFileInventory {
    pub fn new(path: PathBuf) -> TextFileInventory {
        TextFileInventory { path }
    }
}

impl InventorySource for TextFileInventory {
    fn read_inventory(&self) -> Result<Inventory, InventorySourceError> {
        let contents = std::fs::read_to_string(&self.path).map_err(|error| {
            InventorySourceError::Unavailable {
                source: format!("words file '{}'", self.path.display()),
                reason: match error.kind() {
                    ErrorKind::NotFound => "the file does not exist",
                    ErrorKind::PermissionDenied => "permission denied",
                    ErrorKind::InvalidData => "the file is not UTF-8 text",
                    _ => "the file cannot be read",
                }
                .to_string(),
            }
        })?;
        let vocabulary = contents
            .strip_prefix('\u{feff}')
            .unwrap_or(&contents)
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(str::to_string)
            .collect();
        Ok(Inventory { vocabulary })
    }
}

#[cfg(test)]
mod tests;
