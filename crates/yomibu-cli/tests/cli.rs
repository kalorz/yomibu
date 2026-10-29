use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// The default configuration kept at the repository root.
const DEFAULT_CONFIGURATION: &str =
    concat!(env!("CARGO_MANIFEST_DIR"), "/../../yomibu.default.toml");

/// A fresh, empty directory for one test.
fn directory(name: &str) -> PathBuf {
    let directory = std::env::temp_dir().join(format!("yomibu-cli-{}-{name}", std::process::id()));
    if directory.exists() {
        std::fs::remove_dir_all(&directory).expect("stale directory removed");
    }
    std::fs::create_dir_all(&directory).expect("directory created");
    directory
}

/// Runs `yomibu` with `config_home` as `XDG_CONFIG_HOME`, so the real home is never touched.
fn yomibu_in(config_home: &Path, working_directory: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_yomibu"))
        .env("XDG_CONFIG_HOME", config_home)
        .current_dir(working_directory)
        .args(args)
        .output()
        .expect("yomibu runs")
}

fn yomibu(config_home: &Path, args: &[&str]) -> Output {
    yomibu_in(config_home, config_home, args)
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

/// The directory `yomibu story` reads its configuration from under `config_home`.
fn yomibu_directory(config_home: &Path) -> PathBuf {
    config_home.join("yomibu")
}

/// A home holding the default configuration and a words file with two words.
fn configured_home(name: &str) -> PathBuf {
    let config_home = directory(name);
    let yomibu_directory = yomibu_directory(&config_home);
    std::fs::create_dir_all(&yomibu_directory).expect("directory created");
    std::fs::copy(DEFAULT_CONFIGURATION, yomibu_directory.join("yomibu.toml"))
        .expect("configuration copied");
    std::fs::write(yomibu_directory.join("words.txt"), "猫\n犬\n").expect("words written");
    config_home
}

#[test]
fn help_succeeds_and_shows_usage() {
    let output = yomibu(&directory("help"), &["--help"]);
    assert!(output.status.success());
    let help = stdout(&output);
    assert!(help.contains("Usage: yomibu"));
    assert!(help.contains("story"));
}

#[test]
fn no_arguments_shows_usage_and_fails() {
    let output = yomibu(&directory("no-arguments"), &[]);
    assert!(!output.status.success());
    assert!(stderr(&output).contains("Usage: yomibu"));
}

#[test]
fn story_help_explains_the_configuration_file() {
    let output = yomibu(&directory("story-help"), &["story", "--help"]);
    assert!(output.status.success());
    let help = stdout(&output);
    assert!(help.contains("--configuration"));
    assert!(help.contains("~/.config/yomibu/yomibu.toml"));
    assert!(help.contains("XDG_CONFIG_HOME"));
    assert!(help.contains("[infrastructure.inventory]"));
    assert!(help.contains("[[story.generator]]"));
    assert!(help.contains("text_file"));
    assert!(help.contains("echo"));
}

#[test]
fn story_prints_the_candidate_from_the_default_configuration() {
    let config_home = configured_home("default");

    let output = yomibu(&config_home, &["story"]);

    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(stdout(&output), "猫\n犬\n");
}

#[test]
fn story_reads_the_words_file_from_the_configuration_directory() {
    let config_home = configured_home("other-working-directory");
    let elsewhere = directory("elsewhere");

    let output = yomibu_in(&config_home, &elsewhere, &["story"]);

    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(stdout(&output), "猫\n犬\n");
}

#[test]
fn story_names_the_configuration_file_in_a_configuration_error() {
    let config_home = configured_home("invalid-configuration");
    let configuration = yomibu_directory(&config_home).join("yomibu.toml");
    std::fs::write(&configuration, "this is not toml\n").expect("configuration written");

    let output = yomibu(&config_home, &["story"]);

    assert!(!output.status.success());
    assert!(stdout(&output).is_empty());
    let message = stderr(&output);
    assert!(
        message.starts_with(&format!(
            "error: {}: invalid configuration: ",
            configuration.display()
        )),
        "{message}"
    );
    assert!(!message.ends_with("\n\n"), "{message:?}");
}

#[test]
fn story_names_a_missing_default_configuration() {
    let config_home = directory("missing-default");
    let configuration = yomibu_directory(&config_home).join("yomibu.toml");

    let output = yomibu(&config_home, &["story"]);

    assert!(!output.status.success());
    assert!(stdout(&output).is_empty());
    let message = stderr(&output);
    assert!(
        message.contains(&configuration.display().to_string()),
        "{message}"
    );
    assert!(message.contains("cannot read configuration"), "{message}");
}

#[test]
fn story_prints_the_candidate_from_a_named_configuration() {
    let working_directory = directory("named");
    std::fs::copy(DEFAULT_CONFIGURATION, working_directory.join("custom.toml"))
        .expect("configuration copied");
    std::fs::write(working_directory.join("words.txt"), "猫\n").expect("words written");

    let output = yomibu_in(
        &directory("named-home"),
        &working_directory,
        &["story", "--configuration", "custom.toml"],
    );

    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(stdout(&output), "猫\n");
}

#[test]
fn story_names_a_named_configuration_it_cannot_read() {
    let working_directory = directory("missing-named");
    let path = working_directory.join("nonexistent.toml");

    let output = yomibu(
        &working_directory,
        &["story", "--configuration", &path.display().to_string()],
    );

    assert!(!output.status.success());
    assert!(stdout(&output).is_empty());
    let message = stderr(&output);
    assert!(message.contains(&path.display().to_string()), "{message}");
    assert!(message.contains("cannot read configuration"), "{message}");
}

#[test]
fn story_names_a_words_file_it_cannot_read() {
    let config_home = configured_home("missing-words");
    let words = yomibu_directory(&config_home).join("words.txt");
    std::fs::remove_file(&words).expect("words removed");

    let output = yomibu(&config_home, &["story"]);

    assert!(!output.status.success());
    assert!(stdout(&output).is_empty());
    assert!(stderr(&output).contains(&format!("words file '{}'", words.display())));
}
