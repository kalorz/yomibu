use std::{fs, path::Path};

use yomibu::adapters::dictionary::{ManagedInstallation, import_bundle, verify};
use yomibu::{
    adapters::sudachi::SudachiAnalyzer,
    analysis::Sentence,
    evaluation::{EvaluationBindings, evaluate},
    grammar::GrammarDeclarations,
};

fn source_bundle() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/a1/current")
}

#[test]
fn import_verifies_and_copies_a_complete_pinned_bundle() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().join("managed");
    let generation = import_bundle(&root, &source_bundle()).unwrap();
    let published = root.join("bundles").join(&generation);
    assert_ne!(
        published.canonicalize().unwrap(),
        source_bundle().canonicalize().unwrap()
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
    // edit to prove explicit verification and arbitrary-path loading still hash
    // the complete file rather than trusting installation records or metadata.
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
    assert!(verify(&root).is_err());
    assert!(matches!(
        SudachiAnalyzer::load(&path),
        Err(yomibu::adapters::sudachi::DictionaryError::Mismatch)
    ));
}

#[test]
fn an_arbitrary_bundle_is_not_a_managed_installation() {
    assert!(ManagedInstallation::open(source_bundle()).is_err());
    let directory = tempfile::tempdir().unwrap();
    fs::write(directory.path().join("system_core.dic"), b"external").unwrap();
    assert!(ManagedInstallation::open(directory.path()).is_err());
    assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
}

#[test]
fn mapped_and_owned_analysis_and_evaluation_agree_and_old_readers_survive_reimport() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().join("managed");
    let old = import_bundle(&root, &source_bundle()).unwrap();
    let installation = ManagedInstallation::open(&root).unwrap();
    // This test owns the private root and only publishes new generations. It
    // never modifies or truncates the verified generation used by this mapping.
    let mapped = unsafe { SudachiAnalyzer::load_managed(installation) }.unwrap();
    let owned = SudachiAnalyzer::load(source_bundle().join("system_core.dic")).unwrap();
    let input: serde_json::Value =
        serde_json::from_str(include_str!("../../../tests/fixtures/analyze/nominal.json")).unwrap();
    let grammar = GrammarDeclarations::from_descriptions(["です — manual familiarity"]).unwrap();
    let bindings: EvaluationBindings = serde_json::from_value(input["bindings"].clone()).unwrap();
    for text in [
        "犬です。",
        "東京都。猫",
        "犬でした。",
        "猫は水を飲みます。",
        "未知xyz",
        "猫\n犬",
    ] {
        let sentence = Sentence::new(text).unwrap();
        let a = mapped.analyze(sentence.clone()).unwrap();
        let b = owned.analyze(sentence).unwrap();
        assert_eq!(a.sentence, b.sentence);
        assert_eq!(a.units, b.units);
        assert_eq!(
            a.provenance.analyzer_revision,
            b.provenance.analyzer_revision
        );
        assert_eq!(
            a.provenance.dictionary_sha256,
            b.provenance.dictionary_sha256
        );
        assert_eq!(
            a.provenance.configuration_sha256,
            b.provenance.configuration_sha256
        );
        assert_eq!(
            serde_json::to_value(evaluate(&a, &grammar, &bindings).unwrap()).unwrap(),
            serde_json::to_value(evaluate(&b, &grammar, &bindings).unwrap()).unwrap()
        );
        let provenance = serde_json::to_value(&a.provenance).unwrap();
        assert_eq!(provenance["dictionary_loading"]["generation"], old);
        assert_eq!(
            provenance["dictionary_loading"]["verification"],
            "full_sha256_at_installation"
        );
        assert_eq!(
            provenance["dictionary_loading"]["startup_checks"],
            "installation_records_size_and_header"
        );
        assert!(
            serde_json::to_value(&b.provenance)
                .unwrap()
                .get("dictionary_loading")
                .is_none()
        );
    }
    let new = import_bundle(&root, &source_bundle()).unwrap();
    assert_ne!(new, old);
    let current =
        unsafe { SudachiAnalyzer::load_managed(ManagedInstallation::open(&root).unwrap()) }
            .unwrap();
    let sentence = Sentence::new("犬です。").unwrap();
    let old_analysis = mapped.analyze(sentence.clone()).unwrap();
    let new_analysis = current.analyze(sentence).unwrap();
    assert_eq!(old_analysis.units, new_analysis.units);
    assert_eq!(
        serde_json::to_value(new_analysis).unwrap()["provenance"]["dictionary_loading"]["generation"],
        new
    );
    assert!(
        root.join("bundles")
            .join(old)
            .join("system_core.dic")
            .is_file()
    );
}

#[test]
fn owned_loading_keeps_its_verified_snapshot_after_external_file_changes() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("external.dic");
    fs::copy(source_bundle().join("system_core.dic"), &path).unwrap();
    let analyzer = SudachiAnalyzer::load(&path).unwrap();
    let sentence = Sentence::new("東京都。猫").unwrap();
    let before = analyzer.analyze(sentence.clone()).unwrap();
    fs::write(&path, b"externally truncated").unwrap();
    assert_eq!(before, analyzer.analyze(sentence).unwrap());
    assert!(SudachiAnalyzer::load(&path).is_err());
}
