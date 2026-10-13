pub(crate) mod args;
mod auth;
use crate::composition::{
    Credentials, LocalApp, Operation, ServiceEndpoints,
    configuration::{Invocation, components, load_configuration},
};
use crate::{
    commands::args::{Cli, Command},
    composition::credential_input as credentials,
    output,
};
use anyhow::{Context, Result, bail};
use std::{
    io,
    time::{SystemTime, UNIX_EPOCH},
};

pub(crate) fn run(
    cli: Cli,
    endpoints: ServiceEndpoints,
    mut credentials: Credentials,
) -> Result<()> {
    let Some(command) = cli.command else {
        return Ok(());
    };
    let (operation, flags) = match command {
        Command::Auth { credential } => {
            if cli.no_keychain {
                bail!("Credential setup cannot be used with --no-keychain.");
            }
            auth::require_interactive()?;
            let target = if let Some(target) = credential {
                auth::select(&target)?
            } else {
                auth::Target::Missing(components::credentials().collect())
            };
            credentials::environment(&mut credentials, target.requirements());
            if matches!(target, auth::Target::Missing(_)) {
                crate::composition::credentials::install_keychain(
                    &mut credentials,
                    target.requirements(),
                );
            }
            return auth::configure(&target, &credentials);
        }
        Command::Story(args) => (Operation::Story, args.overrides()),
        Command::Sync => (Operation::Sync, Invocation::default()),
    };
    let config = load_configuration(cli.data_dir, cli.config, flags, &operation)?;
    let requirements = [match operation {
        Operation::Story => components::GENERATION_KEY,
        Operation::Sync => components::SOURCE_KEY,
    }];
    credentials::environment(&mut credentials, &requirements);
    if !cli.no_keychain {
        crate::composition::credentials::install_keychain(&mut credentials, &requirements);
    }
    let app = LocalApp::new(config, credentials).with_endpoints(endpoints);
    let clock = SystemTime::now();
    let seed = clock
        .duration_since(UNIX_EPOCH)
        .context("System clock")?
        .as_nanos() as u64;
    let mut out = io::stdout().lock();
    match operation {
        Operation::Story => {
            let data = app.read_learner_data()?;
            let mut progress_error = None;
            let report = runtime()?.block_on(app.story(data, clock.into(), seed, |event| {
                if cli.verbose {
                    let result = output::story::write_progress(&mut out, &event);
                    if let Err(error) = result {
                        progress_error = Some(error);
                    }
                }
            }))?;
            if let Some(error) = progress_error {
                return Err(error.into());
            }
            output::story::write_run(&mut out, &report, cli.verbose)?;
        }
        Operation::Sync => {
            let report = runtime()?.block_on(app.sync())?;
            output::status::write_status(&mut out, &report.summary)?;
        }
    }
    Ok(())
}

fn runtime() -> Result<tokio::runtime::Runtime> {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("I/O runtime")
}
