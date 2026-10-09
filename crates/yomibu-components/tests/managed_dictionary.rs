#[path = "../../../tests/support/dictionary.rs"]
mod test_dictionary;

use std::fs;

use yomibu_components::sudachi_dictionary::{
    SudachiAnalyzer,
    installation::{ManagedInstallation, import_bundle, verify},
};
use yomibu_core::domain::analysis::Sentence;

#[test]
fn verification_after_a_touch_checks_bytes_without_refreshing_installation_evidence() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().join("managed");
    let generation = import_bundle(&root, &test_dictionary::bundle()).unwrap();
    let bundle = root.join("bundles").join(generation);
    for _ in 0..2 {
        drop(ManagedInstallation::open(&root).unwrap());
    }
    let receipt = fs::read(bundle.join("installation.json")).unwrap();
    let selection = fs::read(root.join("current")).unwrap();
    fs::File::open(bundle.join("system_core.dic"))
        .unwrap()
        .set_times(fs::FileTimes::new().set_modified(std::time::SystemTime::UNIX_EPOCH))
        .unwrap();

    assert!(!verify(&root).unwrap().metadata_matches_installation);

    assert_eq!(fs::read(bundle.join("installation.json")).unwrap(), receipt);
    assert_eq!(fs::read(root.join("current")).unwrap(), selection);
    let error = ManagedInstallation::open(&root).err().unwrap();
    assert!(matches!(
        error,
        yomibu_components::sudachi_dictionary::installation::InstallationError::Changed
    ));
    assert!(error.to_string().contains("metadata changed"), "{error}");
    assert!(error.to_string().contains("dictionary verify"), "{error}");
    assert!(error.to_string().contains("reimport"), "{error}");
}

#[test]
fn import_verifies_and_copies_a_complete_pinned_bundle() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().join("managed");
    let generation = import_bundle(&root, &test_dictionary::bundle()).unwrap();
    let published = root.join("bundles").join(&generation);
    assert_ne!(
        published.canonicalize().unwrap(),
        test_dictionary::bundle().canonicalize().unwrap()
    );
    for name in [
        "system_core.dic",
        "LEGAL",
        "LICENSE-2.0.txt",
        "installation.json",
    ] {
        assert!(published.join(name).is_file(), "missing {name}");
    }
    verify(&root).unwrap();
    let installation = ManagedInstallation::open(&root).unwrap();
    assert_eq!(installation.generation(), generation);
    let current: serde_json::Value =
        serde_json::from_slice(&fs::read(root.join("current")).unwrap()).unwrap();
    assert_eq!(current["generation"], generation);
    // No live mapping exists here. Simulate an unsupported same-length external
    // edit to prove explicit verification still hashes the complete file.
    drop(installation);
    let path = published.join("system_core.dic");
    use std::{
        io::{Seek, SeekFrom, Write},
        os::unix::fs::PermissionsExt,
    };
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
    let mut file = fs::OpenOptions::new().write(true).open(&path).unwrap();
    file.seek(SeekFrom::End(-1)).unwrap();
    file.write_all(b"x").unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o400)).unwrap();
    assert!(matches!(
        verify(&root),
        Err(yomibu_components::sudachi_dictionary::installation::InstallationError::Mismatch(name)) if name == "system_core.dic"
    ));
    assert!(matches!(
        ManagedInstallation::open(&root),
        Err(yomibu_components::sudachi_dictionary::installation::InstallationError::Changed)
    ));
}

#[test]
fn an_arbitrary_bundle_is_not_a_managed_installation() {
    assert!(ManagedInstallation::open(test_dictionary::bundle()).is_err());
    let directory = tempfile::tempdir().unwrap();
    fs::write(directory.path().join("system_core.dic"), b"external").unwrap();
    assert!(ManagedInstallation::open(directory.path()).is_err());
    assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
}

#[test]
fn old_readers_keep_their_generation_after_reimport() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().join("managed");
    let old = import_bundle(&root, &test_dictionary::bundle()).unwrap();
    // Only the importer writes here, publishing new generations without touching
    // the verified files used by either mapping.
    let analyzer =
        unsafe { SudachiAnalyzer::load(ManagedInstallation::open(&root).unwrap()) }.unwrap();
    let sentence = Sentence::new("東京都。猫").unwrap();
    let before = analyzer.analyze(sentence.clone()).unwrap();
    assert_eq!(before.provenance.dictionary_loading.generation, old);

    let retained_bundle = root.join("bundles").join(&old);
    let new = import_bundle(&root, &retained_bundle).unwrap();
    assert_ne!(new, old);
    let current =
        unsafe { SudachiAnalyzer::load(ManagedInstallation::open(&root).unwrap()) }.unwrap();
    let old_analysis = analyzer.analyze(sentence.clone()).unwrap();
    let new_analysis = current.analyze(sentence).unwrap();
    assert_eq!(before, old_analysis);
    assert_eq!(old_analysis.units, new_analysis.units);
    assert_eq!(new_analysis.provenance.dictionary_loading.generation, new);
    assert!(
        root.join("bundles")
            .join(old)
            .join("system_core.dic")
            .is_file()
    );
}
