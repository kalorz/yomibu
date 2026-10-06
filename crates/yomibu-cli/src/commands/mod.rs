//! Select a command handler; each handler configures its concrete resources.
use crate::{
    args::{Cli, Command},
    output,
};
use std::io;
use yomibu::{
    adapters::openai::{Client as OpenAiClient, ProviderError},
    story::StoryGenerationOptions,
};

pub(crate) mod analyze;
pub(crate) mod dictionary;
mod input;
pub(crate) mod retrieval;
pub(crate) mod story;
pub(crate) mod sync;

pub(crate) fn run(
    cli: Cli,
    make_client: impl FnOnce(&str) -> Result<OpenAiClient, ProviderError>,
) -> anyhow::Result<()> {
    match cli.command {
        Command::Dictionary { command } => {
            dictionary::run(command, &mut io::stdout().lock()).map_err(output::command_error)?
        }
        Command::PreviewStory {
            story,
            candidates,
            json,
        } => story::preview(
            &story,
            StoryGenerationOptions {
                candidate_count: candidates.get(),
            },
            json,
            &mut io::stdout().lock(),
        )
        .map_err(output::command_error)?,
        Command::PrepareRetrieval { story, embedding } => {
            retrieval::prepare(&story, &embedding, &mut io::stdout().lock())
                .map_err(output::command_error)?
        }
        Command::GenerateStory {
            story,
            embedding,
            dictionary,
            candidates,
            json,
            ..
        } => story::generate(
            &story,
            &embedding,
            StoryGenerationOptions {
                candidate_count: candidates.get(),
            },
            &dictionary,
            json,
            make_client,
            &mut io::stdout().lock(),
        )
        .map_err(output::command_error)?,
        Command::Analyze {
            dictionary,
            input,
            json,
        } => analyze::run(&dictionary, &input, json, &mut io::stdout().lock())
            .map_err(output::command_error)?,
        Command::Sync => sync::sync(cli.data_dir, &mut io::stdout().lock())?,
        Command::Status => sync::status(cli.data_dir, &mut io::stdout().lock())?,
    }
    Ok(())
}
