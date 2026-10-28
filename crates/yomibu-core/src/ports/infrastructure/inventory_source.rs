use std::fmt;

use crate::domain::inventory::Inventory;

/// Implementations are shared across operations and threads.
pub trait InventorySource: Send + Sync {
    /// The whole inventory; a query with limits replaces this when inventories grow.
    fn read_inventory(&self) -> Result<Inventory, InventorySourceError>;
}

#[derive(Debug)]
pub enum InventorySourceError {
    /// `source` names the source as the user would, e.g. `words file 'words.txt'`.
    /// `reason` is translated for the user, never a raw I/O or provider message.
    Unavailable { source: String, reason: String },
}

impl fmt::Display for InventorySourceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            InventorySourceError::Unavailable { source, reason } => {
                write!(f, "{source} is unavailable: {reason}")
            }
        }
    }
}
