use args::Cli;
use clap::Parser;
use std::{
    io::{self, Write},
    process::ExitCode,
};
use yomibu::adapters::openai::{Client as OpenAiClient, ProviderError};

mod args;
mod commands;
mod output;

#[cfg(test)]
mod story_tests;

fn main() -> ExitCode {
    ExitCode::from(entry(std::env::args_os(), OpenAiClient::new))
}

fn entry(
    args: impl IntoIterator<Item = impl Into<std::ffi::OsString> + Clone>,
    make_client: impl FnOnce(&str) -> Result<OpenAiClient, ProviderError>,
) -> u8 {
    let cli = match Cli::try_parse_from(args) {
        Ok(cli) => cli,
        Err(error) => {
            if error.use_stderr() {
                let _ = write!(
                    io::stderr().lock(),
                    "{}",
                    output::escape_argument_error(error)
                );
                return 2;
            }
            return if error.print().is_ok() { 0 } else { 1 };
        }
    };
    match commands::run(cli, make_client) {
        Ok(()) => 0,
        Err(error) => {
            let _ = writeln!(io::stderr().lock(), "error: {error}");
            1
        }
    }
}
