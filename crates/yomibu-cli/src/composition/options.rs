use crate::composition::configuration::{
    OptionOverrides, Patch,
    components::{self, Value},
};
use clap::{Arg, ArgMatches, Args, Command, FromArgMatches};

pub(crate) fn arguments(secret: bool) -> impl Iterator<Item = Arg> {
    components::settings()
        .filter(move |s| s.secret().is_some() == secret)
        .map(move |setting| {
            let name = setting.name();
            let argument = Arg::new(cli_name(name)).long(cli_name(name));
            if secret {
                argument
                    .global(true)
                    .help_heading("Credentials")
                    .value_name("KEY")
                    .help(format!(
                        "Secret credential; env: {}",
                        environment_name(name)
                    ))
            } else {
                argument
                    .help(format!(
                        "{} env: {}",
                        setting.description(),
                        environment_name(name)
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
            if let Some(value) = matches.get_one::<Value>(&cli_name(setting.name())) {
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

pub(crate) fn cli_name(name: yomibu_core::extension::options::OptionName) -> String {
    format!("{}-{}", name.component, name.option)
}

pub(crate) fn environment_name(name: yomibu_core::extension::options::OptionName) -> String {
    format!(
        "YOMIBU_{}",
        cli_name(name).replace('-', "_").to_ascii_uppercase()
    )
}

#[cfg(test)]
mod tests {
    use crate::composition::{
        Operation,
        configuration::{Configuration, ConfigurationInput, components::GENERATION_MODEL},
    };
    use clap::Parser;

    #[derive(Parser)]
    struct AnotherCommand {
        #[command(flatten)]
        story: crate::commands::args::StoryArgs,
    }

    #[test]
    fn flattened_story_arguments_register_read_and_update_component_options() {
        let mut cli =
            AnotherCommand::try_parse_from(["another", "--openai-model", "日本語"]).unwrap();
        cli.try_update_from(["another", "--openai-model", "updated-model"])
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
            Some("updated-model")
        );
    }
}
