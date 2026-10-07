#![allow(dead_code)] // Each test target uses only the resources it needs.

use std::path::{Path, PathBuf};

fn workspace() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("workspace member must be under crates/")
}

pub fn installation() -> PathBuf {
    let root = workspace().join("target/test-resources/managed-dictionary");
    assert!(
        root.join("current").is_file(),
        "Missing managed test dictionary: {}. Run the setup commands in README.md.",
        root.display()
    );
    root
}

pub fn bundle() -> PathBuf {
    let root = workspace();
    let bundle = root.join("target/test-resources/sudachi-core/current");
    for name in ["system_core.dic", "LEGAL", "LICENSE-2.0.txt"] {
        let path = bundle.join(name);
        assert!(
            path.is_file(),
            "Missing test dictionary resource: {}. Run `python3 {}`.",
            path.display(),
            root.join("scripts/setup_test_dictionary.py").display()
        );
    }
    bundle
}
