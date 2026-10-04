use std::path::Path;

use yomibu::{
    adapters::sudachi::{DictionaryError, SudachiAnalyzer},
    analysis::Sentence,
};

#[test]
fn missing_and_unpinned_dictionaries_are_execution_errors() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("system.dic");
    assert!(matches!(
        SudachiAnalyzer::load(&path),
        Err(DictionaryError::Io(_))
    ));
    std::fs::write(&path, b"not the pinned dictionary").unwrap();
    assert!(matches!(
        SudachiAnalyzer::load(&path),
        Err(DictionaryError::Mismatch)
    ));
    // A plausible length must not bypass the exact-byte checksum pin.
    std::fs::OpenOptions::new()
        .write(true)
        .open(&path)
        .unwrap()
        .set_len(217_466_039)
        .unwrap();
    assert!(matches!(
        SudachiAnalyzer::load(&path),
        Err(DictionaryError::Mismatch)
    ));
}

#[test]
fn real_core_dictionary_preserves_whole_compounds_components_and_original_byte_spans() {
    let analyzer = SudachiAnalyzer::load(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("target/a1/current/system_core.dic"),
    )
    .expect("install the pinned Core dictionary using the documented A1 setup");
    let sentence = Sentence::new("東京都。猫").unwrap();
    let analysis = analyzer.analyze(sentence).unwrap();
    assert_eq!(analysis.sentence.text(), "東京都。猫");
    let whole = &analysis.units[0];
    assert_eq!(whole.token.span, 0..9);
    assert_eq!(&sentence.text()[whole.token.span.clone()], "東京都");
    assert_eq!(whole.token.dictionary_form, "東京都");
    let components: Vec<_> = whole
        .components
        .iter()
        .map(|token| &sentence.text()[token.span.clone()])
        .collect();
    assert_eq!(components, ["東京", "都"]);
    assert_eq!(analysis.units.last().unwrap().token.span, 12..15);
    assert!(!whole.token.out_of_vocabulary);
    assert_eq!(
        analysis.provenance.analyzer_revision,
        "90fd6068c80c2fc3b63e0dbab0e341475bad4d8f"
    );
}

#[test]
fn configuration_ignores_ambient_files() {
    if std::env::var_os("YOMIBU_A1_AMBIENT_PROBE").is_some() {
        let analyzer = SudachiAnalyzer::load(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("target/a1/current/system_core.dic"),
        )
        .unwrap();
        let analysis = analyzer
            .analyze(Sentence::new("猫です。").unwrap())
            .unwrap();
        assert_eq!(analysis.units[0].token.dictionary_form, "猫");
        assert_eq!(
            analysis.provenance.configuration_sha256,
            "45cde6f1eba960c32475e267dfa422e51b1f215e3fa142162079d711eff77e4c"
        );
        return;
    }
    let directory = tempfile::tempdir().unwrap();
    for filename in ["sudachi.json", "char.def", "unk.def", "rewrite.def"] {
        std::fs::write(
            directory.path().join(filename),
            b"invalid ambient configuration",
        )
        .unwrap();
    }
    // Only this child receives the marker and working directory; parallel tests
    // and the caller's environment stay independent.
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "configuration_ignores_ambient_files",
            "--nocapture",
        ])
        .current_dir(directory.path())
        .env("YOMIBU_A1_AMBIENT_PROBE", "1")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{} {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
