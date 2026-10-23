use std::process::{Command, Output};

fn yomibu(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_yomibu"))
        .args(args)
        .output()
        .expect("yomibu runs")
}

#[test]
fn help_succeeds_and_shows_usage() {
    let output = yomibu(&["--help"]);
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("Usage: yomibu"));
}

#[test]
fn no_arguments_shows_usage_and_fails() {
    let output = yomibu(&[]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("Usage: yomibu"));
}
