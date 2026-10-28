use std::path::PathBuf;

use yomibu_core::ports::infrastructure::inventory_source::{InventorySource, InventorySourceError};

use super::TextFileInventory;

fn words_file(name: &str, contents: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("yomibu-{}-{name}", std::process::id()));
    std::fs::write(&path, contents).expect("words file written");
    path
}

#[test]
fn reads_one_word_per_line_trimmed_and_without_blank_lines() {
    let path = words_file("words.txt", "猫\n\n 犬 \n");

    let inventory = TextFileInventory::new(path)
        .read_inventory()
        .expect("inventory read");

    assert_eq!(inventory.vocabulary, vec!["猫", "犬"]);
}

#[test]
fn reads_an_empty_file_as_an_empty_inventory() {
    let path = words_file("empty.txt", "");

    let inventory = TextFileInventory::new(path)
        .read_inventory()
        .expect("inventory read");

    assert!(inventory.vocabulary.is_empty());
}

#[test]
fn names_the_file_when_it_cannot_be_read() {
    let path = PathBuf::from("/nonexistent/words.txt");

    let error = TextFileInventory::new(path)
        .read_inventory()
        .expect_err("missing file fails");

    let InventorySourceError::Unavailable { source, reason } = error;
    assert_eq!(source, "words file '/nonexistent/words.txt'");
    assert_eq!(reason, "the file does not exist");
}

#[test]
fn drops_a_byte_order_mark_before_the_first_word() {
    let path = words_file("bom.txt", "\u{feff}猫\n");

    let inventory = TextFileInventory::new(path)
        .read_inventory()
        .expect("inventory read");

    assert_eq!(inventory.vocabulary, vec!["猫"]);
}
