use crate::domain::inventory::Inventory;

/// The prepared inputs bound together for generation.
pub struct Plan {
    pub inventory: Inventory,
}
