pub(crate) mod args;
mod auth;
pub(crate) mod credentials;
mod keychain;
use crate::{
    commands::args::{Cli, Command, DictionaryCommand},
    output,
};
use anyhow::{Context, Result, bail};
use std::{
    collections::BTreeMap,
    io,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};
use yomibu::{
    application::{Credentials, LocalApp, Operation, ServiceEndpoints},
    configuration::{Configuration, ConfigurationInput, ProcessOverrides, environment_names},
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
            auth::require_interactive(cli.json)?;
            if let Some(key) = credential {
                return auth::configure_target(&key);
            }
            let config = load_configuration(
                cli.data_dir,
                cli.config,
                ProcessOverrides::default(),
                &Operation::Auth,
            )?;
            credentials::environment(&mut credentials);
            auth::install(&mut credentials);
            return auth::configure(&config, &credentials);
        }
        Command::Story(args) => (Operation::Story, args.overrides()),
        Command::PreviewStory(args) => (Operation::Preview, args.overrides()),
        Command::PrepareRetrieval(args) => (Operation::Retrieval, args.overrides()),
        Command::Analyze { dictionary, input } => {
            (Operation::Analyze(input), dictionary.overrides())
        }
        Command::Dictionary { command } => {
            let (operation, dictionary_dir) = match command {
                DictionaryCommand::Import {
                    bundle,
                    dictionary_dir,
                } => (Operation::Import(bundle), dictionary_dir),
                DictionaryCommand::Verify { dictionary_dir } => (Operation::Verify, dictionary_dir),
            };
            (
                operation,
                args::DictionaryArgs { dictionary_dir }.overrides(),
            )
        }
        Command::Sync => (Operation::Sync, ProcessOverrides::default()),
        Command::Status => (Operation::Status, ProcessOverrides::default()),
    };
    let config = load_configuration(cli.data_dir, cli.config, flags, &operation)?;
    credentials::environment(&mut credentials);
    if !cli.no_keychain {
        auth::install(&mut credentials);
    }
    let app = LocalApp::new(config, credentials).with_endpoints(endpoints);
    let clock = SystemTime::now();
    let seed = clock
        .duration_since(UNIX_EPOCH)
        .context("System clock")?
        .as_nanos() as u64;
    let mut out = io::stdout().lock();
    let mut err = io::stderr().lock();
    match operation {
        Operation::Auth => bail!("Credential setup must run through yomibu auth."),
        Operation::Story => {
            let mut progress_error = None;
            // Managed roots follow the importer's immutable-generation contract.
            let report = runtime()?.block_on(unsafe {
                app.story(clock.into(), seed, |event| {
                    if cli.verbose {
                        let result = if cli.json {
                            output::story::write_progress(&mut err, &event)
                        } else {
                            output::story::write_progress(&mut out, &event)
                        };
                        if let Err(error) = result {
                            progress_error = Some(error);
                        }
                    }
                })
            })?;
            if let Some(error) = progress_error {
                return Err(error.into());
            }
            output::story::write_run(&mut out, &mut err, &report, cli.json, cli.verbose)?;
        }
        Operation::Preview => output::story::write_preview(
            &mut out,
            &mut err,
            &app.preview(clock.into(), seed)?,
            cli.json,
        )?,
        Operation::Retrieval => {
            let cache = runtime()?.block_on(app.prepare_retrieval(clock.into()))?;
            if cli.json {
                output::write_json(&mut out, &cache)?;
            } else {
                output::story::write_retrieval(&mut out, &cache)?;
            }
        }
        Operation::Analyze(input) => {
            // Managed roots follow the importer's immutable-generation contract.
            let report = unsafe { app.analyze(&input) }?;
            if cli.json {
                output::write_json(&mut out, &report)?;
            } else {
                output::analysis::write_text(&mut out, &report)?;
            }
        }
        Operation::Import(bundle) => {
            output::dictionary::write_import(&mut out, &app.import_dictionary(&bundle)?, cli.json)?
        }
        Operation::Verify => {
            let verification = app.verify_dictionary()?;
            output::dictionary::write_verified(&mut out, &verification, cli.json)?;
        }
        Operation::Sync => {
            let report = runtime()?.block_on(app.sync())?;
            if cli.json {
                output::write_json(&mut out, &report)?;
            } else {
                output::status::write_status(&mut out, &report.summary)?;
            }
        }
        Operation::Status => {
            let summary = app.status()?;
            if cli.json {
                output::write_json(&mut out, &summary)?;
            } else {
                output::status::write_status(&mut out, &summary)?;
            }
        }
    }
    Ok(())
}

fn load_configuration(
    data_dir: Option<PathBuf>,
    config: Option<PathBuf>,
    flags: ProcessOverrides,
    operation: &Operation,
) -> Result<Configuration> {
    let environment: BTreeMap<_, _> = environment_names()
        .into_iter()
        .filter_map(|name| std::env::var(&name).ok().map(|value| (name, value)))
        .collect();
    Ok(Configuration::load(
        ConfigurationInput {
            data_dir: data_dir.or_else(|| std::env::var_os("YOMIBU_DATA_DIR").map(PathBuf::from)),
            config: config.or_else(|| std::env::var_os("YOMIBU_CONFIG").map(PathBuf::from)),
            home: std::env::var_os("HOME")
                .filter(|home| !home.is_empty())
                .map(PathBuf::from),
            environment,
            flags,
        },
        operation,
    )?)
}

fn runtime() -> Result<tokio::runtime::Runtime> {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("I/O runtime")
}
