use std::path::{Path, PathBuf};

pub fn bundle() -> PathBuf {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("workspace member must be under crates/");
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
