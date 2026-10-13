use super::{
    LearnerProgress,
    inventory::{InventoryError, LearnerInventory, MappedContent},
};
use chrono::{DateTime, Utc};

#[derive(Debug)]
pub struct LearnerData {
    pub records: Vec<(MappedContent, Option<LearnerProgress>)>,
    pub access_expires_at: Option<DateTime<Utc>>,
}

impl LearnerData {
    pub fn into_inventory(self, now: DateTime<Utc>) -> Result<LearnerInventory, InventoryError> {
        if self.access_expires_at.is_some_and(|expiry| now >= expiry) {
            return Err(InventoryError::AccessExpired);
        }
        LearnerInventory::from_mapped(self.records)
    }
}
