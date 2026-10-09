use clap::{Arg, ArgMatches};
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

pub(crate) fn read(matches: &ArgMatches) -> Result<OptionOverrides, clap::Error> {
    let mut options = OptionOverrides::default();
    for setting in components::settings().filter(|s| s.secret().is_none()) {
        if let Some(value) = matches.get_one::<Value>(&setting.name().cli()) {
            options
                .insert(setting.name(), Patch::Set(value.clone()))
                .map_err(|message| {
                    clap::Error::raw(clap::error::ErrorKind::InvalidValue, message)
                })?;
        }
    }
    Ok(options)
}
