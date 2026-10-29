//! Command-line entry point. Commands arrive with their tickets; `yomibu --help` lists them.
use std::error::Error;
use std::ffi::OsString;
use std::fmt;
use std::fs::{self, OpenOptions};
use std::io::{self, ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Arg, ArgMatches, Command, value_parser};
use yomibu_app::write_story::WriteStory;
use yomibu_compose::{CompositionRoot, Configuration, KNOWN_GENERATORS, KNOWN_INVENTORY_BACKENDS};
use yomibu_core::domain::candidate::Candidate;

/// The configuration the first run writes, kept at the repository root.
const DEFAULT_CONFIGURATION: &str = include_str!("../../../yomibu.default.toml");
const WORDS_FILE: &str = "words.txt";

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
                        .help(
                            "TOML file naming the inventory backend and the story's components \
                             [default: ~/.config/yomibu/yomibu.toml]",
                        )
                        .long_help(configuration_help()),
                ),
        )
}

fn configuration_help() -> String {
    format!(
        "TOML file with two sections:\n\n{DEFAULT_CONFIGURATION}\n\
         Known inventory backends: {}. Known generators: {}.\n\n\
         Defaults to $XDG_CONFIG_HOME/yomibu/yomibu.toml, or ~/.config/yomibu/yomibu.toml. \
         The first run creates it, with an empty {WORDS_FILE} beside it.",
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
    let location = configuration_location(arguments)?;
    if let ConfigurationLocation::Default(path) = &location
        && !path.exists()
    {
        let set_up =
            set_up(path).map_err(|error| format!("cannot set up {}: {error}", path.display()))?;
        return Err(set_up.to_string());
    }
    let path = location.path();
    let write_story =
        write_story_from_file(path).map_err(|error| format!("{}: {error}", path.display()))?;
    let candidate = write_story.run().map_err(|error| error.to_string())?;
    print_story(&candidate, &mut io::stdout().lock())
}

/// Where `yomibu story` looks for its configuration. Only the default is set up when missing.
enum ConfigurationLocation {
    Default(PathBuf),
    Given(PathBuf),
}

impl ConfigurationLocation {
    fn path(&self) -> &Path {
        match self {
            ConfigurationLocation::Default(path) | ConfigurationLocation::Given(path) => path,
        }
    }
}

fn configuration_location(arguments: &ArgMatches) -> Result<ConfigurationLocation, String> {
    if let Some(path) = arguments.get_one::<PathBuf>("configuration") {
        return Ok(ConfigurationLocation::Given(path.clone()));
    }
    default_configuration_path(
        std::env::var_os("XDG_CONFIG_HOME"),
        std::env::var_os("HOME"),
    )
    .map(ConfigurationLocation::Default)
    .ok_or_else(|| {
        "cannot find the configuration directory: set HOME or XDG_CONFIG_HOME, \
             or pass --configuration"
            .to_string()
    })
}

/// `$XDG_CONFIG_HOME/yomibu/yomibu.toml`, or `~/.config/yomibu/yomibu.toml`. The environment
/// values come in as arguments so the rule is tested without touching the process environment.
fn default_configuration_path(
    xdg_config_home: Option<OsString>,
    home: Option<OsString>,
) -> Option<PathBuf> {
    // The XDG specification says a relative or empty XDG_CONFIG_HOME is ignored.
    let base = match xdg_config_home.filter(|value| Path::new(value).is_absolute()) {
        Some(xdg_config_home) => PathBuf::from(xdg_config_home),
        None => PathBuf::from(home?).join(".config"),
    };
    Some(base.join("yomibu").join("yomibu.toml"))
}

/// The files the first run created, for the message that says where they are.
struct SetUp {
    configuration: PathBuf,
    words: PathBuf,
}

impl fmt::Display for SetUp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "created {} and {}\nadd words to {WORDS_FILE}, one per line, then run `yomibu story` again",
            self.configuration.display(),
            self.words.display()
        )
    }
}

/// Writes the default configuration and an empty words file beside it.
fn set_up(configuration: &Path) -> io::Result<SetUp> {
    let directory = configuration.parent().unwrap_or(Path::new("."));
    fs::create_dir_all(directory)?;
    create_private(configuration, DEFAULT_CONFIGURATION)?;
    let words = directory.join(WORDS_FILE);
    create_private(&words, "")?;
    Ok(SetUp {
        configuration: configuration.to_path_buf(),
        words,
    })
}

/// Creates a file readable by its owner only, and never replaces one that exists.
fn create_private(path: &Path, contents: &str) -> io::Result<()> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options.open(path)?.write_all(contents.as_bytes())
}

/// Every failure here is about the configuration file, so the caller names it once.
/// Relative paths inside the configuration resolve against the file's directory.
fn write_story_from_file(path: &Path) -> Result<WriteStory, Box<dyn Error>> {
    let text =
        fs::read_to_string(path).map_err(|error| format!("cannot read configuration: {error}"))?;
    let configuration: Configuration = text.parse()?;
    let directory = path.parent().unwrap_or(Path::new("."));
    let root = CompositionRoot::new(&configuration.infrastructure, directory)?;
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
