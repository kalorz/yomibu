pub(crate) mod args;
use crate::{
    commands::args::{Cli, Command, DictionaryCommand},
    output,
};
use anyhow::{Context, Result};
use std::{
    collections::BTreeMap,
    io,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};
use yomibu::{
    application::{Credentials, LocalApp, Operation, ServiceEndpoints},
    configuration::{Configuration, ConfigurationInput, ENVIRONMENT_SETTINGS, Settings},
};

pub(crate) fn run(cli: Cli, endpoints: ServiceEndpoints) -> Result<()> {
    let Some(command) = cli.command else {
        return Ok(());
    };
    let (operation, flags) = match command {
        Command::Story(args) => (Operation::Story, args.settings()),
        Command::PreviewStory(args) => (Operation::Preview, args.settings()),
        Command::PrepareRetrieval(args) => (Operation::Retrieval, args.settings()),
        Command::Analyze { dictionary, input } => {
            (Operation::Analyze(input), dictionary.settings())
        }
        Command::Dictionary {
            command:
                DictionaryCommand::Import {
                    bundle,
                    dictionary_dir,
                },
        } => (
            Operation::Import(bundle),
            Settings {
                dictionary_dir,
                ..Default::default()
            },
        ),
        Command::Dictionary {
            command: DictionaryCommand::Verify { dictionary_dir },
        } => (
            Operation::Verify,
            Settings {
                dictionary_dir,
                ..Default::default()
            },
        ),
        Command::Sync => (Operation::Sync, Settings::default()),
        Command::Status => (Operation::Status, Settings::default()),
    };
    let environment: BTreeMap<_, _> = ENVIRONMENT_SETTINGS
        .iter()
        .filter_map(|name| {
            std::env::var(name)
                .ok()
                .map(|value| ((*name).into(), value))
        })
        .collect();
    let config = Configuration::load(
        ConfigurationInput {
            data_dir: cli
                .data_dir
                .or_else(|| std::env::var_os("YOMIBU_DATA_DIR").map(PathBuf::from)),
            config: cli
                .config
                .or_else(|| std::env::var_os("YOMIBU_CONFIG").map(PathBuf::from)),
            home: std::env::var_os("HOME")
                .filter(|home| !home.is_empty())
                .map(PathBuf::from),
            environment,
            flags,
        },
        &operation,
    )?;
    let credentials = Credentials::new(
        cli.wanikani_api_key
            .or_else(|| std::env::var("YOMIBU_WANIKANI_API_KEY").ok()),
        cli.openai_api_key
            .or_else(|| std::env::var("YOMIBU_OPENAI_API_KEY").ok()),
    );
    let app = LocalApp::new(config, credentials).with_endpoints(endpoints);
    let clock = SystemTime::now();
    let seed = clock
        .duration_since(UNIX_EPOCH)
        .context("System clock")?
        .as_nanos() as u64;
    let mut out = io::stdout().lock();
    let mut err = io::stderr().lock();
    match operation {
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
fn runtime() -> Result<tokio::runtime::Runtime> {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("I/O runtime")
}
