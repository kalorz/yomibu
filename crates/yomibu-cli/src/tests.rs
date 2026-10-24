use std::io::{self, ErrorKind, Write};

use yomibu_core::domain::candidate::Candidate;

use super::print_story;

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
