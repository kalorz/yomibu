use std::ffi::OsString;
use std::io::{self, ErrorKind, Write};
use std::path::PathBuf;

use yomibu_core::domain::candidate::Candidate;

use super::{SetUp, default_configuration_path, print_story};

fn os(value: &str) -> Option<OsString> {
    Some(OsString::from(value))
}

#[test]
fn default_configuration_lives_under_xdg_config_home_when_set() {
    assert_eq!(
        default_configuration_path(os("/xdg"), os("/home/me")),
        Some(PathBuf::from("/xdg/yomibu/yomibu.toml"))
    );
}

#[test]
fn default_configuration_falls_back_to_dot_config_in_the_home_directory() {
    let expected = Some(PathBuf::from("/home/me/.config/yomibu/yomibu.toml"));

    assert_eq!(default_configuration_path(None, os("/home/me")), expected);
    assert_eq!(default_configuration_path(os(""), os("/home/me")), expected);
    assert_eq!(
        default_configuration_path(os("relative/xdg"), os("/home/me")),
        expected
    );
}

#[test]
fn default_configuration_is_unknown_without_a_home_directory() {
    assert_eq!(default_configuration_path(None, None), None);
}

#[test]
fn set_up_message_names_both_files_and_the_next_step() {
    let set_up = SetUp {
        configuration: PathBuf::from("/xdg/yomibu/yomibu.toml"),
        words: PathBuf::from("/xdg/yomibu/words.txt"),
    };

    assert_eq!(
        set_up.to_string(),
        "created /xdg/yomibu/yomibu.toml and /xdg/yomibu/words.txt\n\
         add words to words.txt, one per line, then run `yomibu story` again"
    );
}

/// A writer whose reader has gone away, as when `head` closes the pipe.
struct ClosedPipe;

impl Write for ClosedPipe {
    fn write(&mut self, _buffer: &[u8]) -> io::Result<usize> {
        Err(io::Error::from(ErrorKind::BrokenPipe))
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[test]
fn prints_the_candidate_with_a_trailing_newline() {
    let candidate = Candidate {
        text: "猫\n犬".to_string(),
    };
    let mut out = Vec::new();

    print_story(&candidate, &mut out).expect("story printed");

    assert_eq!(String::from_utf8(out).expect("utf-8"), "猫\n犬\n");
}

#[test]
fn treats_a_closed_pipe_as_success() {
    let candidate = Candidate {
        text: "猫".to_string(),
    };

    print_story(&candidate, &mut ClosedPipe).expect("a reader that stops early is not a failure");
}
