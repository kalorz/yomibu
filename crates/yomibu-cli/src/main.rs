//! Command-line entry point. Commands arrive with their tickets; `yomibu --help` lists them.
use clap::Command;

fn cli() -> Command {
    Command::new("yomibu")
        .about("Japanese reading material matched to what you already know")
        .subcommand_required(true)
        .arg_required_else_help(true)
}

fn main() {
    cli().get_matches();
}
