#[path = "../../../tests/support/dictionary.rs"]
mod test_dictionary;

use yomibu::analysis::Sentence;

#[test]
fn stale_fixture_records_explain_how_to_repeat_setup() {
    use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    std::fs::set_permissions(root, std::fs::Permissions::from_mode(0o700)).unwrap();
    std::fs::DirBuilder::new()
        .mode(0o700)
        .create(root.join("bundles"))
        .unwrap();
    let current = root.join("current");
    std::fs::write(
        &current,
        r#"{"version":99,"generation":"core-20260723-v0-stale"}"#,
    )
    .unwrap();
    std::fs::set_permissions(&current, std::fs::Permissions::from_mode(0o600)).unwrap();
    let panic = std::panic::catch_unwind(|| test_dictionary::open_installation(root))
        .err()
        .expect("unsupported fixture should fail");
    let message = panic.downcast_ref::<String>().unwrap();
    assert!(message.contains("dictionary import"), "{message}");
    assert!(message.contains("README.md"), "{message}");
}

#[test]
fn real_core_dictionary_preserves_whole_compounds_components_and_original_byte_spans() {
    let analyzer = test_dictionary::load_analyzer();
    let sentence = Sentence::new("東京都。猫").unwrap();
    let analysis = analyzer.analyze(sentence).unwrap();
    assert_eq!(analysis.sentence.text(), "東京都。猫");
    let whole = &analysis.units[0];
    assert_eq!(whole.token.span, 0..9);
    assert_eq!(
        &analysis.sentence.text()[whole.token.span.clone()],
        "東京都"
    );
    assert_eq!(whole.token.dictionary_form, "東京都");
    let components: Vec<_> = whole
        .components
        .iter()
        .map(|token| &analysis.sentence.text()[token.span.clone()])
        .collect();
    assert_eq!(components, ["東京", "都"]);
    assert_eq!(analysis.units.last().unwrap().token.span, 12..15);
    assert!(!whole.token.out_of_vocabulary);
    assert_eq!(
        analysis.provenance.analyzer_revision,
        "90fd6068c80c2fc3b63e0dbab0e341475bad4d8f"
    );
    let provenance = serde_json::to_value(&analysis.provenance).unwrap();
    assert_eq!(
        provenance["dictionary_loading"]["verification"],
        "full_sha256_at_installation"
    );
    assert!(
        provenance["dictionary_loading"]["generation"]
            .as_str()
            .unwrap()
            .starts_with("core-20260723-v0-")
    );
}

#[test]
fn configuration_ignores_ambient_files() {
    if std::env::var_os("YOMIBU_SUDACHI_AMBIENT_PROBE").is_some() {
        let analyzer = test_dictionary::load_analyzer();
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
        .env("YOMIBU_SUDACHI_AMBIENT_PROBE", "1")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{} {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
