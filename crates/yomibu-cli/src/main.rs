use clap::{CommandFactory, FromArgMatches};
use commands::args::{Cli, Command};
use std::{
    io::{self, Write},
    process::ExitCode,
};
use yomibu::{application::ServiceEndpoints, configuration::modules::MODULES};

mod commands;
mod output;

#[cfg(test)]
mod story_tests;

fn main() -> ExitCode {
    ExitCode::from(entry(std::env::args_os(), ServiceEndpoints::default()))
}

fn entry(
    args: impl IntoIterator<Item = impl Into<std::ffi::OsString> + Clone>,
    endpoints: ServiceEndpoints,
) -> u8 {
    let guidance = MODULES
        .iter()
        .map(|module| format!("{}: {}\n  {}", module.name, module.purpose, module.guidance))
        .collect::<Vec<_>>()
        .join("\n");
    let (args, credentials) = commands::credentials::capture(args);
    let mut command = Cli::command()
        .args(commands::options::arguments(true))
        .mut_subcommand("story", |command| command.after_help(guidance));
    for name in ["story", "preview-story", "prepare-retrieval"] {
        command = command.mut_subcommand(name, |command| {
            command.args(commands::options::arguments(false))
        });
    }
    let cli = match command.try_get_matches_from_mut(args).and_then(|matches| {
        let mut cli = Cli::from_arg_matches(&matches)?;
        if let Some(
            Command::Story(args) | Command::PreviewStory(args) | Command::PrepareRetrieval(args),
        ) = &mut cli.command
            && let Some((_, matches)) = matches.subcommand()
        {
            args.options = commands::options::read(matches)?;
        }
        Ok(cli)
    }) {
        Ok(cli) => cli,
        Err(mut error) => {
            if error.use_stderr() {
                if error.get(clap::error::ContextKind::Usage).is_none() {
                    error.insert(
                        clap::error::ContextKind::Usage,
                        clap::error::ContextValue::StyledStr(command.render_usage()),
                    );
                }
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
    if cli.command.is_none() {
        return if command.print_help().is_ok() { 0 } else { 1 };
    }
    match commands::run(cli, endpoints, credentials).map_err(output::command_error) {
        Ok(()) => 0,
        Err(error) => {
            let _ = writeln!(io::stderr().lock(), "error: {error}");
            1
        }
    }
}

#[cfg(test)]
#[path = "../../../tests/support/dictionary.rs"]
mod test_dictionary;
