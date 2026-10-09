use clap::{Arg, ArgMatches, Args, Command, FromArgMatches};
use yomibu::configuration::{
    OptionOverrides, Patch,
    components::{self, Value},
};

pub(crate) fn arguments(secret: bool) -> impl Iterator<Item = Arg> {
    components::settings()
        .filter(move |s| s.secret().is_some() == secret)
        .map(move |setting| {
            let name = setting.name();
            let argument = Arg::new(name.cli()).long(name.cli());
            if secret {
                argument
                    .global(true)
                    .help_heading("Credentials")
                    .value_name("KEY")
                    .help(format!("Secret credential; env: {}", name.environment()))
            } else {
                argument
                    .help(format!(
                        "{} env: {}",
                        setting.description(),
                        name.environment()
                    ))
                    .value_name("VALUE")
                    .value_parser(move |text: &str| setting.parse(text))
            }
        })
}

#[derive(Default)]
pub(crate) struct ComponentArgs(pub OptionOverrides);

impl Args for ComponentArgs {
    fn augment_args(command: Command) -> Command {
        command.args(arguments(false))
    }

    fn augment_args_for_update(command: Command) -> Command {
        Self::augment_args(command)
    }
}

impl FromArgMatches for ComponentArgs {
    fn from_arg_matches(matches: &ArgMatches) -> Result<Self, clap::Error> {
        let mut options = Self::default();
        options.update_from_arg_matches(matches)?;
        Ok(options)
    }

    fn update_from_arg_matches(&mut self, matches: &ArgMatches) -> Result<(), clap::Error> {
        for setting in components::options() {
            if let Some(value) = matches.get_one::<Value>(&setting.name().cli()) {
                self.0
                    .insert(setting.name(), Patch::Set(value.clone()))
                    .map_err(|message| {
                        clap::Error::raw(clap::error::ErrorKind::InvalidValue, message)
                    })?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use clap::Parser;
    use yomibu::{
        application::Operation,
        configuration::{
            Configuration, ConfigurationInput,
            components::{EMBEDDING_DIMENSIONS, GENERATION_MODEL},
        },
    };

    #[derive(Parser)]
    struct AnotherCommand {
        #[command(flatten)]
        story: super::super::args::StoryArgs,
    }

    #[test]
    fn flattened_story_arguments_register_read_and_update_component_options() {
        let mut cli = AnotherCommand::try_parse_from([
            "another",
            "--openai-model",
            "日本語",
            "--http-embeddings-dimensions",
            "8",
        ])
        .unwrap();
        cli.try_update_from(["another", "--http-embeddings-dimensions", "16"])
            .unwrap();
        let dir = tempfile::tempdir().unwrap();
        let config = Configuration::load(
            ConfigurationInput {
                data_dir: Some(dir.path().into()),
                flags: cli.story.overrides(),
                home: None,
                config: None,
                environment: Default::default(),
            },
            &Operation::Story,
        )
        .unwrap();
        assert_eq!(
            config
                .pipeline
                .options
                .get(GENERATION_MODEL)
                .unwrap()
                .map(String::as_str),
            Some("日本語")
        );
        assert_eq!(
            config.pipeline.options.get(EMBEDDING_DIMENSIONS).unwrap(),
            Some(&16)
        );
    }
}
