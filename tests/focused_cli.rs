use serde_json::{Value, json};
use std::{
    fs,
    path::Path,
    process::{Command, Output},
};
const INPUT: &str = include_str!("fixtures/focused/pet-rest.json");
fn cli(dir: &Path, command: &str) -> Command {
    let mut c = Command::new(env!("CARGO_BIN_EXE_yomibu"));
    c.env_clear().current_dir(dir).args([
        command,
        "--permissions",
        "permissions.json",
        "--focus-entry",
        "1",
        "--data-dir",
        "ignored",
    ]);
    c
}
fn diagnostic(o: Output, code: i32, part: &str) -> String {
    assert_eq!(
        o.status.code(),
        Some(code),
        "{}",
        String::from_utf8_lossy(&o.stderr)
    );
    assert!(
        o.stdout.is_empty(),
        "{}",
        String::from_utf8_lossy(&o.stdout)
    );
    let text = String::from_utf8(o.stderr).unwrap();
    assert!(text.starts_with("error:"));
    assert!(text.contains(part), "{text}");
    text
}
#[test]
fn optional_preview_is_offline_readable_and_reports_exact_outbound_content() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("permissions.json"), INPUT).unwrap();
    for path in [".env", "wanikani.json", "grammar.json", "sudachi.json"] {
        fs::write(dir.path().join(path), "poison").unwrap();
    }
    let before = fs::read_dir(dir.path()).unwrap().count();
    let output = cli(dir.path(), "context-preview")
        .arg("--json")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["version"], 1);
    assert_eq!(report["kind"], "focused_context_preview");
    assert_eq!(report["status"], "ready");
    assert_eq!(report["selector_revision"], "g2-situations-v1");
    assert_eq!(report["focus_entry"], 1);
    assert_eq!(report["situations"].as_array().unwrap().len(), 3);
    let body = report["request"]["body_utf8"].as_str().unwrap();
    assert_eq!(
        body.len(),
        report["request"]["bytes"].as_u64().unwrap() as usize
    );
    assert!(body.contains("猫"));
    assert!(!body.contains("犬"));
    let o = cli(dir.path(), "context-preview").output().unwrap();
    assert!(o.status.success());
    assert!(o.stderr.is_empty());
    let text = String::from_utf8(o.stdout).unwrap();
    assert!(text.starts_with("Experimental focused context — no generation performed\nFocus #1: 寝る / ネル / sleep\n"),"{text}");
    for line in [
        "Situation: pet-rest — A pet rests. Describe the selected pet sleeping.",
        "Selected #1: 寝る — explicit focus; predicate",
        "Selected #2: 猫 — required participant; first available declared alternative",
        "Excluded #3: 犬 — participant alternative not needed",
        "Excluded #4: 歩く — outside the chosen situation",
        "Required grammar: TopicWa (#1), PoliteNonPast (#2)",
        "Requests made: 0",
    ] {
        assert!(text.contains(&format!("{line}\n")), "{text}");
    }
    assert!(text.contains(&serde_json::to_string(body).unwrap()));
    assert_eq!(fs::read_dir(dir.path()).unwrap().count(), before);
}
#[test]
fn argument_and_unavailable_context_exits_preserve_layout() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("permissions.json"), INPUT).unwrap();
    let text = diagnostic(
        cli(dir.path(), "generate-focused")
            .args(["--dictionary", "missing.dic"])
            .output()
            .unwrap(),
        2,
        "--allow-model-call",
    );
    assert!(text.contains("\n\nUsage: yomibu generate-focused"));
    for args in [
        vec!["--help"],
        vec!["--version"],
        vec!["context-preview", "--help"],
        vec!["generate-focused", "--help"],
    ] {
        let o = Command::new(env!("CARGO_BIN_EXE_yomibu"))
            .env_clear()
            .args(args)
            .output()
            .unwrap();
        assert!(o.status.success());
        assert!(o.stderr.is_empty());
        assert!(!o.stdout.is_empty());
    }
    for id in ["0", "-1", "cat\n\u{1b}"] {
        let o = Command::new(env!("CARGO_BIN_EXE_yomibu"))
            .env_clear()
            .args([
                "context-preview",
                "--permissions",
                "missing",
                "--focus-entry",
                id,
            ])
            .output()
            .unwrap();
        let text = diagnostic(o, 2, "error:");
        assert!(!text.contains('\u{1b}'));
        assert!(text.contains("\n\n"));
    }
    let mut v: Value = serde_json::from_str(INPUT).unwrap();
    v["bindings"]["grammar"] = json!([]);
    fs::write(dir.path().join("permissions.json"), v.to_string()).unwrap();
    for command in ["context-preview", "generate-focused"] {
        let mut c = cli(dir.path(), command);
        c.arg("--json");
        if command == "generate-focused" {
            c.args(["--allow-model-call", "--dictionary", "missing.dic"]);
        }
        let o = c.output().unwrap();
        assert_eq!(o.status.code(), Some(1));
        let r: Value = serde_json::from_slice(&o.stdout).unwrap();
        assert_eq!(r["status"], "unavailable");
        assert!(r["request"].is_null());
        assert!(
            String::from_utf8(o.stderr)
                .unwrap()
                .contains("context is unavailable")
        );
    }
}
#[test]
fn input_file_limit_is_exact_and_bad_inputs_are_rejected_before_dictionary_or_credentials() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("permissions.json");
    let mut exact = INPUT.as_bytes().to_vec();
    exact.resize(4_194_304, b' ');
    fs::write(&path, &exact).unwrap();
    assert!(
        cli(dir.path(), "context-preview")
            .arg("--json")
            .output()
            .unwrap()
            .status
            .success()
    );
    exact.push(b' ');
    fs::write(&path, &exact).unwrap();
    diagnostic(
        cli(dir.path(), "context-preview").output().unwrap(),
        1,
        "4194304 bytes",
    );
    let mut bad: Vec<Vec<u8>> = vec![
        vec![0xff],
        b"{".to_vec(),
        INPUT
            .replace("\"version\": 1", "\"version\": 2")
            .into_bytes(),
        INPUT
            .replace("\"version\": 1", "\"version\": 1, \"version\": 1")
            .into_bytes(),
        INPUT
            .replace("\"version\": 1", "\"version\": 1, \"extra\": false")
            .into_bytes(),
    ];
    let value: Value = serde_json::from_str(INPUT).unwrap();
    for key in ["version", "grammar", "bindings"] {
        let mut v = value.clone();
        v.as_object_mut().unwrap().remove(key);
        bad.push(v.to_string().into_bytes());
    }
    for key in ["written_form", "reading", "sense", "direct_object"] {
        let mut v = value.clone();
        v["bindings"]["vocabulary"][3]
            .as_object_mut()
            .unwrap()
            .remove(key);
        bad.push(v.to_string().into_bytes());
    }
    for key in ["declaration_id", "rule"] {
        let mut v = value.clone();
        v["bindings"]["grammar"][1]
            .as_object_mut()
            .unwrap()
            .remove(key);
        bad.push(v.to_string().into_bytes());
    }
    let mut v = value.clone();
    v["grammar"][0] = json!(" \n");
    bad.push(v.to_string().into_bytes());
    let mut v = value.clone();
    v["bindings"]["vocabulary"][3]["sense"] = json!("x".repeat(1025));
    bad.push(v.to_string().into_bytes());
    let mut v = value.clone();
    v["grammar"] = json!(vec!["\u{1b}".repeat(1024); 3]);
    bad.push(v.to_string().into_bytes());
    for bytes in bad {
        fs::write(&path, bytes).unwrap();
        let o = cli(dir.path(), "generate-focused")
            .args(["--allow-model-call", "--dictionary", "missing.dic"])
            .output()
            .unwrap();
        let text = diagnostic(o, 1, "error:");
        assert!(!text.contains("Dictionary initialization"));
        assert!(!text.contains("OPENAI_API_KEY"));
    }
    fs::write(&path, INPUT).unwrap();
    diagnostic(
        cli(dir.path(), "generate-focused")
            .args(["--allow-model-call", "--dictionary", "missing.dic"])
            .output()
            .unwrap(),
        1,
        "Dictionary initialization",
    );
}
#[test]
fn hostile_fields_stay_escaped_without_changing_decoded_strings_or_japanese() {
    let dir = tempfile::tempdir().unwrap();
    let mut v: Value = serde_json::from_str(INPUT).unwrap();
    let hostile = "日本語\nInjected\u{1b}\r\t\u{7f}\u{9b}\u{202e}\u{2028}\u{e0001}";
    v["grammar"].as_array_mut().unwrap().push(json!(hostile));
    v["bindings"]["vocabulary"][3]["written_form"] = json!(hostile);
    fs::write(dir.path().join("permissions.json"), v.to_string()).unwrap();
    for json in [true, false] {
        let mut c = cli(dir.path(), "context-preview");
        if json {
            c.arg("--json");
        }
        let o = c.output().unwrap();
        assert!(o.status.success());
        assert!(o.stderr.is_empty());
        let text = String::from_utf8(o.stdout).unwrap();
        for ch in [
            '\u{1b}',
            '\r',
            '\t',
            '\u{7f}',
            '\u{9b}',
            '\u{202e}',
            '\u{2028}',
            '\u{e0001}',
        ] {
            assert!(!text.contains(ch));
        }
        assert!(text.contains("日本語"));
        if json {
            let r: Value = serde_json::from_str(&text).unwrap();
            assert_eq!(r["grammar"][2], hostile);
        } else {
            assert!(text.contains("\nRequests made: 0\n"));
            assert!(!text.contains("\nInjected"));
        }
    }
}
