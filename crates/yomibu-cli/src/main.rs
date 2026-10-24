//! Command-line entry point. Commands arrive with their tickets; `yomibu --help` lists them.
use std::error::Error;
use std::io::{self, ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Arg, ArgMatches, Command, value_parser};
use yomibu_app::write_story::WriteStory;
use yomibu_compose::{CompositionRoot, Configuration, KNOWN_GENERATORS, KNOWN_INVENTORY_BACKENDS};
use yomibu_core::domain::candidate::Candidate;

fn cli() -> Command {
    Command::new("yomibu")
        .about("Japanese reading material matched to what you already know")
        .subcommand_required(true)
        .arg_required_else_help(true)
        .subcommand(
            Command::new("story")
                .about("Write a story from the configured inventory")
                .arg(
                    Arg::new("configuration")
                        .long("configuration")
                        .value_name("PATH")
                        .value_parser(value_parser!(PathBuf))
                        .default_value("yomibu.toml")
                        .help("TOML file naming the inventory backend and the story's components")
                        .long_help(configuration_help()),
                ),
        )
}

fn configuration_help() -> String {
    format!(
        "TOML file with two sections:\n\n\
         [infrastructure.inventory]   # where the words come from\n\
         backend = \"text_file\"\n\
         path = \"words.txt\"         # one word per line\n\n\
         [[story.generator]]          # which generator writes the story\n\
         component = \"echo\"\n\n\
         Known inventory backends: {}. Known generators: {}.",
        KNOWN_INVENTORY_BACKENDS.join(", "),
        KNOWN_GENERATORS.join(", ")
    )
}

fn main() -> ExitCode {
    let matches = cli().get_matches();
    let result = match matches.subcommand() {
        Some(("story", arguments)) => story(arguments),
        _ => unreachable!("clap requires a known subcommand"),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("error: {message}");
            ExitCode::FAILURE
        }
    }
}

fn story(arguments: &ArgMatches) -> Result<(), String> {
    let path: &PathBuf = arguments
        .get_one("configuration")
        .expect("configuration has a default");
    let write_story =
        write_story_from_file(path).map_err(|error| format!("{}: {error}", path.display()))?;
    let candidate = write_story.run().map_err(|error| error.to_string())?;
    print_story(&candidate, &mut io::stdout().lock())
}

/// Every failure here is about the configuration file, so the caller names it once.
fn write_story_from_file(path: &Path) -> Result<WriteStory, Box<dyn Error>> {
    let text = std::fs::read_to_string(path)
        .map_err(|error| format!("cannot read configuration: {error}"))?;
    let configuration: Configuration = text.parse()?;
    let root = CompositionRoot::new(&configuration.infrastructure)?;
    Ok(root.build_write_story(&configuration.story)?)
}

/// A reader that stops early, such as `head`, closes the pipe; that is not a failure.
fn print_story(candidate: &Candidate, out: &mut impl Write) -> Result<(), String> {
    match writeln!(out, "{}", candidate.text) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == ErrorKind::BrokenPipe => Ok(()),
        Err(error) => Err(format!("cannot write the story: {error}")),
    }
}

#[cfg(test)]
mod tests;
