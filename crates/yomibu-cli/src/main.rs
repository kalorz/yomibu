mod composition;
mod output;

use std::io;
use std::process::ExitCode;

use clap::{Arg, ArgAction, Command, value_parser};
use uuid::Uuid;
use yomibu_core::domain::LearnerId;

fn cli() -> Command {
    Command::new("yomibu")
        .about("Japanese reading material tools")
        .subcommand_required(true)
        .disable_help_subcommand(true)
        .subcommand(
            Command::new("create-learner")
                .about("Create a learner and print its assigned UUID")
                .after_help("Requires YOMIBU_DATABASE_URL and a database with the retained schema. Surrounding name whitespace is trimmed; blank names are rejected. Names need not be unique. Prints the assigned UUID followed by a newline. With --json, prints {\"id\":\"<UUID>\"} followed by a newline. An interrupted write may have been saved; it is never automatically retried. If creation is confirmed but a later step fails, stderr includes its UUID.")
                .arg(
                    Arg::new("name")
                        .long("name")
                        .value_name("NAME")
                        .help("Name for the new learner")
                        .required(true),
                )
                .arg(
                    Arg::new("json")
                        .long("json")
                        .action(ArgAction::SetTrue)
                        .help("Print the assigned UUID as a JSON object"),
                ),
        )
        .subcommand(
            Command::new("collections")
                .about("List collection UUIDs associated with a learner")
                .after_help("Requires YOMIBU_DATABASE_URL and a database with the retained schema. By default, prints one UUID per line; no associations produce no output. With --json, prints a JSON array of UUID strings followed by a newline; no associations produce []. Ordering is unspecified in both formats.")
                .arg(
                    Arg::new("learner")
                        .long("learner")
                        .value_name("UUID")
                        .help("Learner whose associated collections to list")
                        .required(true)
                        .value_parser(value_parser!(Uuid)),
                )
                .arg(
                    Arg::new("json")
                        .long("json")
                        .action(ArgAction::SetTrue)
                        .help("Print collection UUIDs as a JSON array"),
                ),
        )
}

fn main() -> ExitCode {
    // clap handles help and rejects syntax before configuration or database access.
    let matches = cli().get_matches();
    match matches.subcommand() {
        Some(("create-learner", create)) => {
            let name = create.get_one::<String>("name").expect("required name");
            output::report_created(
                composition::run_create_learner(name),
                create.get_flag("json"),
                io::BufWriter::new(io::stdout().lock()),
                io::stderr().lock(),
            )
        }
        Some(("collections", collections)) => {
            let learner = *collections
                .get_one::<Uuid>("learner")
                .expect("required learner UUID");
            output::report_collections(
                composition::run_collections(LearnerId(learner)),
                collections.get_flag("json"),
                io::BufWriter::new(io::stdout().lock()),
                io::stderr().lock(),
            )
        }
        _ => unreachable!("clap requires a known subcommand"),
    }
}
