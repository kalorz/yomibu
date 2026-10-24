use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn yomibu(args: &[&str]) -> Output {
    yomibu_in(Path::new("."), args)
}

fn yomibu_in(directory: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_yomibu"))
        .current_dir(directory)
        .args(args)
        .output()
        .expect("yomibu runs")
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

/// The example shipped at the repository root, which echoes `words.txt`.
const EXAMPLE_CONFIGURATION: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../yomibu.toml");

/// A directory holding `words.txt` and the example `yomibu.toml`.
fn configured_directory(name: &str) -> PathBuf {
    let directory = std::env::temp_dir().join(format!("yomibu-cli-{}-{name}", std::process::id()));
    std::fs::create_dir_all(&directory).expect("directory created");
    std::fs::write(directory.join("words.txt"), "猫\n犬\n").expect("words written");
    std::fs::copy(EXAMPLE_CONFIGURATION, directory.join("yomibu.toml"))
        .expect("configuration copied");
    directory
}

#[test]
fn help_succeeds_and_shows_usage() {
    let output = yomibu(&["--help"]);
    assert!(output.status.success());
    let help = stdout(&output);
    assert!(help.contains("Usage: yomibu"));
    assert!(help.contains("story"));
}

#[test]
fn no_arguments_shows_usage_and_fails() {
    let output = yomibu(&[]);
    assert!(!output.status.success());
    assert!(stderr(&output).contains("Usage: yomibu"));
}

#[test]
fn story_help_explains_the_configuration_file() {
    let output = yomibu(&["story", "--help"]);
    assert!(output.status.success());
    let help = stdout(&output);
    assert!(help.contains("--configuration"));
    assert!(help.contains("yomibu.toml"));
    assert!(help.contains("[infrastructure.inventory]"));
    assert!(help.contains("[[story.generator]]"));
    assert!(help.contains("text_file"));
    assert!(help.contains("echo"));
}

#[test]
fn story_names_the_configuration_file_in_a_configuration_error() {
    let directory = configured_directory("invalid-configuration");
    std::fs::write(directory.join("yomibu.toml"), "this is not toml\n")
        .expect("configuration written");

    let output = yomibu_in(&directory, &["story"]);

    assert!(!output.status.success());
    assert!(stdout(&output).is_empty());
    let message = stderr(&output);
    assert!(
        message.starts_with("error: yomibu.toml: invalid configuration: "),
        "{message}"
    );
    assert!(!message.ends_with("\n\n"), "{message:?}");
}

#[test]
fn story_prints_the_candidate_from_the_default_configuration() {
    let directory = configured_directory("default");

    let output = yomibu_in(&directory, &["story"]);

    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(stdout(&output), "猫\n犬\n");
}

#[test]
fn story_prints_the_candidate_from_a_named_configuration() {
    let directory = configured_directory("named");
    std::fs::rename(directory.join("yomibu.toml"), directory.join("custom.toml"))
        .expect("configuration renamed");

    let output = yomibu_in(&directory, &["story", "--configuration", "custom.toml"]);

    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(stdout(&output), "猫\n犬\n");
}

#[test]
fn story_names_a_configuration_file_it_cannot_read() {
    let output = yomibu(&["story", "--configuration", "/nonexistent/yomibu.toml"]);

    assert!(!output.status.success());
    assert!(stdout(&output).is_empty());
    assert!(stderr(&output).contains("/nonexistent/yomibu.toml"));
}

#[test]
fn story_names_a_words_file_it_cannot_read() {
    let directory = configured_directory("missing-words");
    std::fs::remove_file(directory.join("words.txt")).expect("words removed");

    let output = yomibu_in(&directory, &["story"]);

    assert!(!output.status.success());
    assert!(stdout(&output).is_empty());
    assert!(stderr(&output).contains("words file 'words.txt'"));
}
